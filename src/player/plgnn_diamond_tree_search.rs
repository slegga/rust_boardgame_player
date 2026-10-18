/*
* LOGIC GATE NEURAL NETWORK!
*
* PUCT-MCTS tree search around the LG-Diamond "brain":
*  - arena Vec<Node> with parent pointers: O(1) backup (no O(n) parent scan)
*  - PUCT selection: Q + PUCT_C * prior * sqrt(parent_N) / (1 + n)
*  - leaf value = BrainDiamond::evaluate_bit_state clamped to [0,1]
*    (no random rollout; terminal states score 1.0/0.0/0.5 directly,
*    so a forced win -> 1.0 as soon as the winning move is expanded)
*  - negamax values: node.wins accumulates from node.player's perspective
*    (the player to move at that node); backup negates per level (0.5 is
*    symmetric under negation)
*  - moves stored as Vec<u8> columns; String API preserved in get_move
*  - tree kept between moves, re-rooted through a transposition table keyed
*    on the bit_state hash. All stored states are from `me_color`'s
*    perspective, so hash equality really means the same position.
*  - a child link is (move, node): the same node can hang under several
*    parents/legs by transposition, so the move identity lives on the
*    link, never on the node
*  - optional root expansion batch across threads behind feature `ts_par`
*    (single-threaded by default).
*
* bit_state convention (unchanged assumption from the old code,
* connect4-style): [4]=draw [5]=perspective wins [6]=opponent wins, and
* move strings are numeric ("1".."7"). Games returning non-numeric move
* strings degrade to an index fallback (only the root level then maps
* back to the real move strings).
*/
use crate::player::brain::lg_diamond::*;
use crate::player::brain::{BitOp, LogicGatesError, NO_LAYERS, CELL_SIZE};
use crate::player::{Agentish, Brainy};
use boardgame_game::game::get_active_player_from_bit_state;
use boardgame_game::game::get_terminal_state_from_bit_state;
use boardgame_game::game::{GameStatic, Playable, TerminalState};
use log::{info, warn};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

const PUCT_C: f32 = 1.4; // exploration constant, Q + C*prior*sqrt(N)/(1+n)
const DEFAULT_SEC_TO_MOVE: u64 = 15;
const MIN_ITERATIONS: u32 = 256; // floor per move, even with tiny/no time budget
const MAX_ITERATIONS: u32 = 100_000; // caps memory/CPU on top of the time budget
const MAX_ARENA_NODES: usize = 256 * 1024; // soft reset of the reusable tree
const NO_PARENT: usize = usize::MAX;
#[cfg(feature = "ts_par")]
const MIN_BATCH_FOR_PAR: usize = 4; // minimum untried moves before a parallel batch pays off

#[derive(Clone, Debug)]
struct Node {
    visits: u32,
    /// accumulated value from `player`'s (player to move at this node) perspective
    wins: f32,
    /// brain preference for selecting among siblings, from the parent mover's perspective
    prior: f32,
    /// valid columns (u8) not expanded yet
    untried: Vec<u8>,
    /// (move column, node) links; the column is the leg that owns this link
    children: Vec<(u8, usize)>,
    parent: usize,
    /// true when me_color is the player to move at this node; tree-edge
    /// parity ONLY (turn bits are unreliable on terminal states)
    player: bool,
    /// always from me_color's perspective
    bit_state: Vec<bool>,
    is_terminal: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SearchTree {
    arena: Vec<Node>,
    tt: HashMap<u64, usize>,
    root: usize,
    /// set only when a game's move strings are non-numeric: list index -> string
    root_move_strings: Option<Vec<String>>,
}

/// Borrowed context of one search run.
struct Ctx<'a, T: Playable> {
    game: &'a T,
    me_color: &'a str,
    brain: &'a BrainDiamond,
}

fn hash_state(state: &[bool]) -> u64 {
    let mut hasher = DefaultHasher::new();
    state.hash(&mut hasher);
    hasher.finish()
}

/// col -> move string. Index fallback only when the game used non-numeric strings.
fn col_string(fallback: &Option<Vec<String>>, col: u8) -> String {
    match fallback {
        Some(list) => list
            .get(col as usize)
            .cloned()
            .unwrap_or_else(|| col.to_string()),
        None => col.to_string(),
    }
}

impl SearchTree {
    fn clear(&mut self) {
        self.arena.clear();
        self.tt.clear();
        self.root = 0;
        self.root_move_strings = None;
    }

    fn reset_if_needed(&mut self) {
        if self.arena.len() >= MAX_ARENA_NODES {
            warn!("LG tree search: arena full, resetting reusable tree");
            self.clear();
        }
    }

    /// Re-root on a transposition hit, otherwise create a fresh root.
    fn ensure_root(&mut self, root_state: &[bool], untried: Vec<u8>, player_is_me: bool) {
        let h = hash_state(root_state);
        if let Some(&idx) = self.tt.get(&h) {
            if idx < self.arena.len() && self.arena[idx].bit_state.as_slice() == root_state {
                self.arena[idx].parent = NO_PARENT;
                self.root = idx;
                return;
            }
        }
        let node = Node {
            visits: 0,
            wins: 0.0,
            prior: 0.5,
            untried,
            children: Vec::new(),
            parent: NO_PARENT,
            player: player_is_me,
            bit_state: root_state.to_vec(),
            is_terminal: false,
        };
        self.arena.push(node);
        self.root = self.arena.len() - 1;
        self.tt.insert(h, self.root);
    }

    /// Parse valid move strings to u8 columns. Non-numeric strings fall back
    /// to list indices (recorded so root-level expansions map back to strings).
    fn parse_columns(&mut self, valid_moves: &[String]) -> Vec<u8> {
        if valid_moves.iter().all(|m| m.parse::<u8>().is_ok()) {
            return valid_moves
                .iter()
                .filter_map(|m| m.parse::<u8>().ok())
                .collect();
        }
        warn!("Tree search: non numeric move strings, using index fallback");
        self.root_move_strings = Some(valid_moves.to_vec());
        (0..valid_moves.len()).map(|i| i as u8).collect()
    }

    /// Link a leg under `parent` unless that leg is already linked.
    fn link_child(&mut self, parent: usize, col: u8, idx: usize) {
        let children = &mut self.arena[parent].children;
        if !children.iter().any(|&(c, _)| c == col) {
            children.push((col, idx));
        }
    }

    /// PUCT selection among a fully expanded node's children. The Q term is
    /// from the parent mover's perspective (hence 1.0 - child value).
    fn best_child(&self, parent: usize) -> usize {
        let children = &self.arena[parent].children;
        let sqrt_n = (self.arena[parent].visits as f32).sqrt();
        let mut best = children[0].1;
        let mut best_score = f32::MIN;
        for &(_, c) in children {
            let child = &self.arena[c];
            let q = if child.visits > 0 {
                1.0 - child.wins / child.visits as f32
            } else {
                child.prior
            };
            let u = PUCT_C * child.prior * sqrt_n / (1.0 + child.visits as f32);
            if q + u > best_score {
                best_score = q + u;
                best = c;
            }
        }
        best
    }

    /// Negamax backup with parent pointers: O(depth). Value negates once per
    /// level; node.player parity guarantees proper alternation (each tree
    /// edge is one move), so the flip is unconditional.
    fn backprop(&mut self, mut node: usize, mut value: f32) {
        loop {
            let n = &mut self.arena[node];
            n.visits += 1;
            n.wins += value;
            let parent = n.parent;
            if parent == NO_PARENT {
                return;
            }
            value = 1.0 - value;
            node = parent;
        }
    }

    /// Value of a state from `player_is_me`'s perspective. Terminal states
    /// score directly (win -> 1.0, loss -> 0.0, draw -> 0.5); otherwise the
    /// brain eval, clamped to [0,1].
    fn state_value<T: Playable>(ctx: &Ctx<T>, state: &[bool], player_is_me: bool) -> f32 {
        let owned = state.to_vec();
        match get_terminal_state_from_bit_state(&owned) {
            Some(TerminalState::Me) => {
                if player_is_me {
                    1.0
                } else {
                    0.0
                }
            }
            Some(TerminalState::Opponent) => {
                if player_is_me {
                    0.0
                } else {
                    1.0
                }
            }
            Some(TerminalState::Draw) => 0.5,
            None => {
                let eval = ctx.brain.evaluate_bit_state(&owned);
                let v = (eval as f32 / CELL_SIZE as f32).clamp(0.0, 1.0);
                if player_is_me {
                    v
                } else {
                    1.0 - v
                }
            }
        }
    }

    /// Register a computed child state under `parent`, reusing a transposition
    /// hit when present. Returns (node index, value from the child player's
    /// perspective) for backup.
    fn add_child<T: Playable>(
        &mut self,
        ctx: &Ctx<T>,
        parent: usize,
        col: u8,
        child_state: Vec<bool>,
    ) -> (usize, f32) {
        let h = hash_state(&child_state);
        if let Some(&idx) = self.tt.get(&h) {
            if self.arena[idx].bit_state.as_slice() == child_state.as_slice() {
                let child = &self.arena[idx];
                let value = if child.visits > 0 {
                    child.wins / child.visits as f32
                } else {
                    child.prior
                };
                self.link_child(parent, col, idx);
                return (idx, value);
            }
        }
        let is_terminal = get_terminal_state_from_bit_state(&child_state).is_some();
        // Tree edge parity: every edge in the tree is exactly one move, so
        // the player to move toggles every level. Do NOT use the state's turn
        // bits here: Connect4 does not flip the turn bit on the final
        // (winning) move, which would break the negamax alternation.
        let child_is_me = !self.arena[parent].player;
        let value = Self::state_value(ctx, &child_state, child_is_me);
        let untried: Vec<u8> = if is_terminal {
            // terminal nodes have nothing to expand
            Vec::new()
        } else {
            ctx.game
                .get_valid_moves_from_bit_state(ctx.me_color, &child_state)
                .iter()
                .filter_map(|m| m.parse::<u8>().ok())
                .collect()
        };
        // The two players alternate, so the parent mover's perspective is
        // the negation of the child player's perspective.
        let prior = 1.0 - value;
        let node = Node {
            visits: 0,
            wins: 0.0,
            prior,
            untried,
            children: Vec::new(),
            parent,
            player: child_is_me,
            bit_state: child_state.clone(),
            is_terminal,
        };
        self.arena.push(node);
        let idx = self.arena.len() - 1;
        self.tt.entry(h).or_insert(idx);
        self.arena[parent].children.push((col, idx));
        (idx, value)
    }

    /// Expand one untried column under `parent` and evaluate it.
    fn expand_one<T: Playable>(&mut self, ctx: &Ctx<T>, parent: usize, col: u8) -> (usize, f32) {
        let bit_state = self.arena[parent].bit_state.clone();
        let child_state = ctx.game.get_bit_state_from_bit_state_and_move(
            ctx.me_color,
            &bit_state,
            &col_string(&self.root_move_strings, col),
        );
        self.add_child(ctx, parent, col, child_state)
    }

    /// One selection/expansion/backup pass.
    fn iterate_once<T: Playable>(&mut self, ctx: &Ctx<T>) {
        let mut node = self.root;
        loop {
            if self.arena[node].is_terminal {
                // terminal leaf: its stable value is wins/visits
                let n = &self.arena[node];
                let v = if n.visits > 0 {
                    n.wins / n.visits as f32
                } else {
                    n.prior
                };
                self.backprop(node, v);
                return;
            }
            let col = { self.arena[node].untried.pop() };
            if let Some(col) = col {
                let (child, value) = self.expand_one(ctx, node, col);
                self.backprop(child, value);
                return;
            }
            if self.arena[node].children.is_empty() {
                // stalemate: not terminal, no moves, no children. Defensive.
                self.backprop(node, 0.5);
                return;
            }
            node = self.best_child(node);
        }
    }

    /// Optional root expansion batch across threads (feature `ts_par`).
    /// Evaluating one move is the dominant cost, so batching the root's
    /// untried moves is the cheap win. Foreign games need not be `Sync`: all
    /// spawned threads only read the game through `&self` methods and are
    /// all joined before get_move returns.
    #[cfg(feature = "ts_par")]
    fn expand_root_batch_par<T: Playable>(&mut self, ctx: &Ctx<T>) {
        struct ShareGame<'a, T: Playable>(&'a T);
        // Safety: only touched through `&self` (shared reads) inside a
        // std::thread::scope whose handles are joined before returning.
        unsafe impl<'a, T: Playable> Sync for ShareGame<'a, T> {}
        unsafe impl<'a, T: Playable> Send for ShareGame<'a, T> {}

        let fallback = self.root_move_strings.clone();
        let root_state = self.arena[self.root].bit_state.clone();
        let game = ShareGame(ctx.game);
        let cols: Vec<u8> = self.arena[self.root].untried.drain(..).collect();
        if cols.len() < MIN_BATCH_FOR_PAR {
            self.arena[self.root].untried = cols;
            return;
        }
        let results: Vec<(u8, Vec<bool>)> = std::thread::scope(|s| {
            let handles: Vec<_> = cols
                .iter()
                .map(|&col| {
                    let game = &game;
                    let me_color = ctx.me_color;
                    let root_state = root_state.clone();
                    let fallback = fallback.clone();
                    s.spawn(move || {
                        let child_state = game.0.get_bit_state_from_bit_state_and_move(
                            me_color,
                            &root_state,
                            &col_string(&fallback, col),
                        );
                        (col, child_state)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (col, child_state) in results {
            let (child, value) = self.add_child(ctx, self.root, col, child_state);
            self.backprop(child, value);
        }
    }

    /// Best move = most visited root child leg (ties by accumulated value).
    /// Returns (child node, leg column).
    fn best_root_move(&self) -> Option<(usize, u8)> {
        let children = &self.arena[self.root].children;
        children
            .iter()
            .copied()
            .max_by_key(|&(_, c)| {
                let n = &self.arena[c];
                (n.visits, (n.wins * 1024.0) as u32)
            })
            .map(|(col, idx)| (idx, col))
    }
}

#[derive(Clone, Debug)]
pub struct PlayerNNDiamondTS {
    pub name: String,
    pub is_loaded: bool,
    pub brain: BrainDiamond,
    pub sec_to_move: u64,
    pub me_color: String,
    pub opponent_color: String,
    /// reusable PUCT tree; public only so `..Default::default()` struct
    /// updates keep working in the binaries
    pub tree: RefCell<SearchTree>,
}

impl Default for PlayerNNDiamondTS {
    /// remember to set game name after this is used.
    fn default() -> PlayerNNDiamondTS {
        PlayerNNDiamondTS {
            name: "LG Diamond Tree search".to_string(),
            is_loaded: false,
            brain: BrainDiamond::default(),
            sec_to_move: DEFAULT_SEC_TO_MOVE,
            me_color: String::new(),
            opponent_color: String::new(),
            tree: RefCell::default(),
        }
    }
}

impl Agentish for PlayerNNDiamondTS {
    fn get_name(&self) -> String {
        self.name.to_string()
    }

    fn get_move<T: Playable>(&self, _moves: &[String], player: &str, game: &T) -> Option<String> {
        let move_start = Instant::now();
        let me_color: String = if self.me_color.is_empty() {
            player.to_string()
        } else {
            self.me_color.clone()
        };

        let mut tree = self.tree.borrow_mut();
        tree.reset_if_needed();

        let root_state = game.get_bit_state(&me_color);
        if get_terminal_state_from_bit_state(&root_state).is_some() {
            return None; // game over, nothing to move (was: panic)
        }
        let valid_strings = game.get_valid_moves_from_bit_state(&me_color, &root_state);
        if valid_strings.is_empty() {
            return None; // no move available (was: panic)
        }
        let root_cols = tree.parse_columns(&valid_strings);
        let game_static = game.get_game_static();
        let player_is_me =
            get_active_player_from_bit_state(&game_static, &root_state) == me_color;
        tree.ensure_root(&root_state, root_cols, player_is_me);
        let ctx = Ctx {
            game,
            me_color: &me_color,
            brain: &self.brain,
        };

        #[cfg(feature = "ts_par")]
        tree.expand_root_batch_par(&ctx);

        let mut rounds = 0;
        while rounds < MIN_ITERATIONS
            || (rounds < MAX_ITERATIONS && self.has_time_left(&move_start))
        {
            rounds += 1;
            tree.iterate_once(&ctx);
        }

        let (child_idx, leg_col) = tree.best_root_move()?;
        let child = &tree.arena[child_idx];
        let me_value = if tree.arena[tree.root].player {
            child.wins / child.visits as f32
        } else {
            1.0 - child.wins / child.visits as f32
        };
        let cmove = col_string(&tree.root_move_strings, leg_col);
        let (visits, root_visits) = (child.visits, tree.arena[tree.root].visits);
        drop(tree);
        info!(
            "I({}) move {} score: {:.1}% visits: {}/{} rounds: {}",
            self.name, cmove, me_value * 100.0, visits, root_visits, rounds
        );
        Some(cmove)
    }

    fn get_ready(
        &mut self,
        game_static: &GameStatic,
        me_color: &str,
        sec_to_move: u64,
    ) -> Result<(), LogicGatesError> {
        self.sec_to_move = sec_to_move;
        if !self.is_loaded {
            let mut brain = BrainDiamond {
                game_name: game_static.name.clone(),
                ..Default::default()
            };
            brain.game_name = game_static.name.clone();
            match brain.from_file() {
                Ok(()) => {}
                Err(LogicGatesError::Io { source })
                    if source.kind() == std::io::ErrorKind::NotFound =>
                {
                    warn!("File not found loading brain. Generate a new one");
                    brain = generate_random_brain(game_static);
                    brain.save_to_file()?;
                }
                Err(other) => return Err(other), // was: panic
            }
            self.brain = brain;
            self.is_loaded = true;
        }
        self.me_color = me_color.to_string();
        for x in game_static.players.iter() {
            let x_value = x.to_string();
            if x_value != self.me_color {
                self.opponent_color = x_value;
            }
        }
        Ok(())
    }
}

impl Brainy for PlayerNNDiamondTS {
    type MyBrain = BrainDiamond;

    /// Evaluate a position by running the tree search. Returns the value of
    /// the current position from the active player's perspective (0..CELL_SIZE),
    /// like the old implementation.
    fn evaluate_bit_state<T: Playable>(&self, game: &T, bit_state: &Vec<bool>) -> usize {
        let game_static = game.get_game_static();
        let player = get_active_player_from_bit_state(&game_static, bit_state);
        let me_color = if self.me_color.is_empty() {
            player.clone()
        } else {
            self.me_color.clone()
        };
        let move_start = Instant::now();
        let mut tree = self.tree.borrow_mut();
        tree.reset_if_needed();
        // all tree states are from me_color's perspective
        let root_state = game.get_bit_state(&me_color);
        if get_terminal_state_from_bit_state(&root_state).is_some() {
            return self.brain.evaluate_bit_state(&root_state);
        }
        let valid_strings = game.get_valid_moves_from_bit_state(&me_color, &root_state);
        if valid_strings.is_empty() {
            return self.brain.evaluate_bit_state(&root_state);
        }
        let root_cols = tree.parse_columns(&valid_strings);
        tree.ensure_root(&root_state, root_cols, me_color == player);
        let ctx = Ctx {
            game,
            me_color: &me_color,
            brain: &self.brain,
        };
        let mut rounds = 0;
        while rounds < MIN_ITERATIONS
            || (rounds < MAX_ITERATIONS && self.has_time_left(&move_start))
        {
            rounds += 1;
            tree.iterate_once(&ctx);
        }
        let root_node = &tree.arena[tree.root];
        // root_node.wins is from the mover's perspective; flip if the mover
        // is not me_color
        let value = if root_node.player {
            root_node.wins / root_node.visits as f32
        } else {
            1.0 - root_node.wins / root_node.visits as f32
        };
        info!(
            "I({}) evaluated score: {:.1}% rounds: {}",
            self.name,
            value * 100.0,
            rounds
        );
        (value * CELL_SIZE as f32).round() as usize
    }

    fn get_brain(&self) -> Self::MyBrain {
        self.brain.clone()
    }
}

impl PlayerNNDiamondTS {
    fn has_time_left(&self, move_start: &Instant) -> bool {
        Duration::from_secs(self.sec_to_move) >= move_start.elapsed()
    }

    pub fn transform_to_mocked_version(&mut self) {
        // find last layer
        // sett it to be CELL_SIZE/2 +1
        for gate in 0..=(CELL_SIZE / 2) {
            self.brain.layers[NO_LAYERS - 1][0].operator[gate] = BitOp::TRUE;
        }
        for gate in (CELL_SIZE / 2 + 1)..CELL_SIZE {
            self.brain.layers[NO_LAYERS - 1][0].operator[gate] = BitOp::FALSE;
        }

        // set time to think to 0
        self.sec_to_move = 0;
    }
}

#[cfg(test)]
mod tests {
    use crate::player::plgnn_diamond_tree_search::PlayerNNDiamondTS;
    use crate::player::Agentish;
    use boardgame_game::game::connect4::Connect4;
    use boardgame_game::game::Playable;
    use std::time::Instant;

    #[test]
    fn test_if_able_to_block_opponent_from_winning() {
        let mut game = Connect4::default();
        game.play("1");
        game.play("2");
        game.play("1");
        game.play("2");
        game.play("7");
        game.play("2");
        let mut mockai = PlayerNNDiamondTS::default();
        mockai.transform_to_mocked_version();
        assert_eq!(game.get_active_player(), "Red", "Correct player set");
        let moves = game.get_valid_moves();
        assert_eq!(moves.len(), 7, "Correct length");
        let should_be_two = mockai.get_move(&moves, "Red", &game).unwrap();
        assert_eq!(
            should_be_two, "2",
            "tree search manage to block opponent"
        );
    }

    #[test]
    fn test_tree_is_reused_between_moves() {
        // after a second search the arena must be bigger than one fresh root
        let mut game = Connect4::default();
        game.play("1");
        let mut ai = PlayerNNDiamondTS::default();
        ai.transform_to_mocked_version();
        let moves = game.get_valid_moves();
        let m1 = ai.get_move(&moves, "Red", &game).unwrap();
        game.play(&m1);
        let arena1 = ai.tree.borrow().arena.len();
        let moves = game.get_valid_moves();
        let _m2 = ai.get_move(&moves, "Yellow", &game).unwrap();
        let arena2 = ai.tree.borrow().arena.len();
        assert!(arena2 > arena1, "tree must be kept and grown between moves");
    }

    #[test]
    #[ignore]
    fn bench_full_games() {
        use crate::player::random::PlayerRandom;
        let mut game = Connect4::default();
        let _gs = game.get_game_static();
        let mut ai = PlayerNNDiamondTS::default();
        ai.transform_to_mocked_version();
        let rnd = PlayerRandom {
            name: "rnd".into(),
            name_in_game: "Yellow".into(),
        };
        let start = Instant::now();
        let mut games = 0u32;
        let mut total_moves = 0u32;
        while start.elapsed().as_millis() < 5000 {
            games += 1;
            game.reset();
            let mut history: Vec<String> = Vec::new();
            while !game.is_terminal() {
                let moves = game.get_valid_moves();
                if moves.is_empty() {
                    break;
                }
                let mover = game.get_active_player();
                let mv = if mover == "Red" {
                    ai.get_move(&moves, "Red", &game).unwrap()
                } else {
                    rnd.get_move(&moves, "Yellow", &game).unwrap()
                };
                assert!(
                    moves.contains(&mv),
                    "illegal move {} by {} after {:?}",
                    mv,
                    mover,
                    history
                );
                history.push(mv.clone());
                game.play(&mv);
                total_moves += 1;
            }
        }
        println!(
            "BENCH games={} moves={} time={:.2}s ({:.1} moves/s, arena {} nodes)",
            games,
            total_moves,
            start.elapsed().as_secs_f32(),
            total_moves as f32 / start.elapsed().as_secs_f32(),
            ai.tree.borrow().arena.len()
        );
    }
}
