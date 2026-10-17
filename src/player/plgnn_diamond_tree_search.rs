/*
* LOGIC GATE NEURAL NETWORK!
*/
use crate::player::brain::lg_diamond::*;
use crate::player::brain::*;
use crate::player::Agentish;
use crate::player::Brainy;

use boardgame_game::game::get_active_player_from_bit_state;
use log::{debug, info, trace, warn};
use boardgame_game::game::get_terminal_state_from_bit_state;
use boardgame_game::game::GameStatic;
use boardgame_game::game::Playable;
use log::Level::{Info, Trace};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct TSNode {
    moves: HashMap<String, usize>,
    score: usize,
    best_move: Option<String>,
    bit_state: Vec<bool>,
    is_open: bool,
    level: u8,
    player: String,
    score_level: u8,
    score_address: usize,
}

#[derive(Clone, Debug)]
pub struct PlayerNNDiamondTS {
    pub name: String,
    pub is_loaded: bool,
    pub brain: BrainDiamond,
    pub sec_to_move: u64,
    pub me_color: String,
    pub opponent_color: String,
}

impl Default for PlayerNNDiamondTS {
    /// remember to set game name after this is used.
    fn default() -> PlayerNNDiamondTS {
        //            let mut brain = BrainDiamond::default();
        //            brain.game_name = game_static.name.clone();
        PlayerNNDiamondTS {
            name: "LG Diamond Tree search".to_string(),
            is_loaded: false,
            brain: BrainDiamond::default(),
            //   states: vec![TSNode::default()],
            sec_to_move: 15,
            me_color: "".to_string(),
            opponent_color: "".to_string(),
        }
        //            let _ = myself.get_ready(game_static, player_color);
        //            myself
    }
}

impl Agentish for PlayerNNDiamondTS {
    fn get_name(&self) -> String {
        self.name.to_string()
    }

    fn get_move<T: Playable>(&self, _moves: &[String], player: &str, game: &T) -> Option<String> {
        // temporary until ai is working
        let mut states = vec![];
        let bit_state = game.get_bit_state(player);
        let current_score = self.brain.evaluate_bit_state(&bit_state);
        states.push(TSNode {
            moves: HashMap::new(),
            score: current_score,
            best_move: None,
            bit_state,
            is_open: true,
            level: 0,
            player: player.to_string(),
            score_level: 0,
            score_address: 0,
        });
        let move_start = Instant::now();

        // set color if not set

        // loop until timeout
        let mut must_rounds: i32 = 56; // to debug way not right scoring
        while self.has_time_left(&move_start) || must_rounds > 0 {
            // selection
            // get best leaf chose. Variable to set penalty for
            let (leaf_no, leaf_data, is_complete) = self.select_leaf(&states); // return address to most interesting leaf and data.
            if is_complete {
                info!("is_complete. Exit.");
                break;
                //} else if leaf_no == 7 {
                //    println!("Want to brake");
            }
            let new_leaf_data =
                self.explore(&mut states, player, &leaf_no, &leaf_data.unwrap(), game);
            self.backpropagate(&mut states, player, &leaf_no, &new_leaf_data);
            must_rounds -= 1;
        }

        let (cmove, cscore, score_level, _score_address) =
            self.get_best_move_and_score(&states, &0);
        if cmove.is_empty() {
            panic!("Did not find a move for root. {:#?}", states[0]);
        }
        self.dump_states(&states);
        info!(
            "I({}) move {} with score: {} {:.1} level:{} % ",
            self.name,
            cmove,
            cscore,
            cscore as f32 * 100.0 / CELL_SIZE as f32,
            score_level,
            //    states[0]
        );

        Some(cmove.to_string())
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
            //todo!("Look for file or generate a random brain");
            match brain.from_file() {
                Ok(_value) => (),
                Err(e) => match e {
                    LogicGatesError::Io { source: x } => {
                        if x.kind() == std::io::ErrorKind::NotFound {
                            warn!("File not found loading brain. Generate a new one");
                            brain = generate_random_brain(game_static);
                            brain.save_to_file()?;
                        };
                    }
                    other_error => panic!("Problem creating the file: {other_error:?}"),
                },
            }
            // warn!("BrainDiamond is {:?}", brain);
            self.brain = brain;
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
    fn evaluate_bit_state<T: Playable>(&self, game: &T, bit_state: &Vec<bool>) -> usize {

        let mut states = vec![];

        let game_static = game.get_game_static();
        let player = get_active_player_from_bit_state(&game_static, bit_state);
        let bit_state = game.get_bit_state(player.as_str());
        let current_score = self.brain.evaluate_bit_state(&bit_state);
        states.push(TSNode {
            moves: HashMap::new(),
            score: current_score,
            best_move: None,
            bit_state: bit_state.to_vec(),
            is_open: true,
            level: 0,
            player: get_active_player_from_bit_state(&game_static, &bit_state),
            score_level: 0,
            score_address: 0,
        });
        let move_start = Instant::now();
        let player = states[0].player.clone();
        let mut must_rounds: i32 = 56; // to debug way not right scoring
        while self.has_time_left(&move_start) || must_rounds > 0 {
            // selection
            // get best leaf chose. Variable to set penalty for
            let (leaf_no, leaf_data, is_complete) = self.select_leaf(&states); // return address to most interesting leaf and data.
            if is_complete {
                info!("is_complete. Exit.");
                break;
                //} else if leaf_no == 7 {
                //    println!("Want to brake");
            }
            let new_leaf_data =
                self.explore(&mut states, &player, &leaf_no, &leaf_data.unwrap(), game);
            self.backpropagate(&mut states, &player, &leaf_no, &new_leaf_data);
            must_rounds -= 1;
        }
        let (cmove, cscore, score_level, _score_address) =
            self.get_best_move_and_score(&states, &0);
        if cmove.is_empty() {
            panic!("Did not find a move for root. {:#?}", states[0]);
        }
        self.dump_states(&states);
        info!(
            "I({}) move {} with score: {} {:.1} level:{} % ",
            self.name,
            cmove,
            cscore,
            cscore as f32 * 100.0 / CELL_SIZE as f32,
            score_level,
            //    states[0]
        );

        cscore
    }

    type MyBrain = BrainDiamond ;
    fn get_brain(&self) -> Self::MyBrain {
        self.brain.clone()
    }
}

impl PlayerNNDiamondTS {
    fn has_time_left(&self, move_start: &Instant) -> bool {
        Duration::from_secs(self.sec_to_move) >= move_start.elapsed()
    }

    /// Look for states with highest calculated value, with moves.is_empty() == true
    fn select_leaf(&self, states: &[TSNode]) -> (usize, Option<TSNode>, bool) {
        let mut best_value: f64 = -1000000000.0;
        let mut best_address = 100000000;
        let mut best_candidate: Option<TSNode> = None;
        let mut is_complete = true;
        let exploration = 2.0; // low = bredde først, high = dubde først: 1.0= slå meg, 256 lett å slå
                               // let mut do_continue = false;

        // Først undersøker om best_move er åpen
        // Den kan være lukket når videre utforskning av best_move gir samme vurdering som før.
        // Da vil denne logikken gå videre å finne en mer tilfeldig state å explore.
        let address = states[states[0].score_address].score_address;
        if !states[address].is_open {
            if log::log_enabled!(Trace) {
                self.dump_state(&states[address]);
                self.dump_states(states);

                trace!(
                    "leaf is not open adr:{} first_adr:{} ",
                    address,
                    states[0].score_address
                );
            }
        } else if address != states[address].score_address {
            //else for: if is_open == false
            return (
                states[address].score_address,
                Some(states[states[address].score_address].clone()),
                false,
            );
        }

        // Else contiue find the best candidate among all the states
        for (address, cand) in states.iter().enumerate() {
            //for cand in states {
            if cand.moves.is_empty() && cand.is_open && cand.moves.is_empty() {
                is_complete = false;
                if best_candidate.is_none() {
                    best_address = address;
                    best_candidate = Some(cand.clone());
                    best_value = match cand.player == self.opponent_color {
                        // the move choosing player is me
                        true => {
                            cand.score as f64 - (cand.level as f64 * CELL_SIZE as f64 / exploration)
                        }
                        // the move choosing player is opponent
                        false => {
                            (CELL_SIZE - cand.score) as f64
                                - (cand.level as f64 * CELL_SIZE as f64 / exploration)
                        }
                    };
                } else if cand.player == self.opponent_color // the current move choosing player is me
                    && best_value
                        < cand.score as f64 - (cand.level as f64 * CELL_SIZE as f64 / exploration)
                {
                    best_address = address;
                    best_candidate = Some(cand.clone());
                    best_value =
                        cand.score as f64 - (cand.level as f64 * CELL_SIZE as f64 / exploration);
                } else if cand.player == self.me_color // the current move choosing player is opponent
                    && best_value
                        < (CELL_SIZE - cand.score) as f64
                            - (cand.level as f64 * CELL_SIZE as f64 / exploration)
                {
                    best_address = address;
                    best_candidate = Some(cand.clone());
                    best_value = (CELL_SIZE - cand.score) as f64
                        - (cand.level as f64 * CELL_SIZE as f64 / exploration);
                }
            }
            //address += 1;
        }

        if is_complete {
            debug!("Is complete. Number of states {}", states.len());
            self.dump_states(states);
        }
        if best_candidate.is_none() && !is_complete {
            println!("Values address:{} best_value:{best_value}", address);
            panic!("Candidate is None");
        }
        (best_address, best_candidate, is_complete)
    }

    fn explore<T: Playable>(
        &self,
        states: &mut Vec<TSNode>,
        perspective: &str,
        leaf_no: &usize,
        leaf_data: &TSNode,
        game: &T,
    ) -> TSNode {
        if !leaf_data.is_open {
            println!("Error with state address: {}", leaf_no);
            self.dump_state(leaf_data);

            panic!("Try to explore closed leaf");
        }
        let mut new_leaf_data = leaf_data.clone();
        let mut new_score: Option<usize> = None;
        let valid_moves: Vec<String> =
            game.get_valid_moves_from_bit_state(perspective, &leaf_data.bit_state);

        let mut move_alternatives: HashMap<String, usize> = HashMap::new();

        for mov in valid_moves {
            let xmove = mov.clone();

            // println!("{:?} ", leaf_data.bit_state);

            let tmp_state = game.get_bit_state_from_bit_state_and_move(
                perspective,
                &leaf_data.bit_state,
                &xmove.clone(),
            );
            // println!("{}", xmove.as_str());
            let tmp_score = self.brain.evaluate_bit_state(&tmp_state);
            let is_open = get_terminal_state_from_bit_state(&tmp_state).is_none();

            let adr = self.add_leaf(states, &tmp_score, &tmp_state, leaf_data, is_open);
            //panic!("{}", xmove.as_str());
            states[adr].score_address = adr; // after register address.
            move_alternatives.entry(xmove).or_insert(adr);
            if new_score.is_none()
                || (new_leaf_data.player == perspective && tmp_score > new_score.unwrap())
                || (new_leaf_data.player != perspective && tmp_score < new_score.unwrap())
            {
                new_score = Some(tmp_score);
                new_leaf_data.best_move = Some(mov.clone());
                new_leaf_data.score_address = adr;
            }
        }
        if let Some(new_score) = new_score {
            new_leaf_data.moves = move_alternatives;
            new_leaf_data.is_open = false;
            new_leaf_data.score = new_score;
            new_leaf_data.score_level = new_leaf_data.level + 1;
            states[*leaf_no] = new_leaf_data.clone();
            debug!(
                "backpr from explore l:{} {:4}->{:4}:{:3} ::{}",
                new_leaf_data.level,
                new_leaf_data.score_address,
                leaf_no,
                new_leaf_data.score,
                new_leaf_data.score_address
            );
            self.backpropagate(
                states,
                perspective, //states[adr].player.clone().as_str(),
                &new_leaf_data.score_address,
                &new_leaf_data.clone(),
            );
        }
        new_leaf_data
    }

    /// Look for parent node and update parent score and best_move
    fn backpropagate(
        &self,
        states: &mut Vec<TSNode>,
        player: &str, // thinking player/me
        leaf_no: &usize,
        new_leaf_data: &TSNode,
    ) {
        let mut adr = 0;
        //return; // TODO!
        if *leaf_no == 0 {
            return; // no parent
        }
        'outer: loop {
            if adr >= states.len() {
                break;
            }

            for mov in states[adr].moves.clone().into_keys() {
                if states[adr].moves.get(&mov).unwrap() == leaf_no {
                    if adr == 0 {
                        debug!("I want to debug");
                    }
                    // Check if score is up or down.
                    if states[adr].player == player {
                        // We are the choosing player
                        match states[adr].score.cmp(&new_leaf_data.score) {
                            Ordering::Less => {
                                states[adr].score = new_leaf_data.score;
                                states[adr].score_level = new_leaf_data.score_level;
                                states[adr].score_address = new_leaf_data.score_address;
                            }
                            Ordering::Greater => {
                                let (best_move, best_score, best_score_level, score_address) =
                                    self.get_best_move_and_score(states, &adr);
                                states[adr].best_move = Some(best_move);
                                states[adr].score = best_score;
                                states[adr].score_level = best_score_level;
                                states[adr].score_address = score_address;
                            }
                            Ordering::Equal => {
                                // nothing
                                break 'outer;
                            }
                        }
                        if states[adr].best_move.as_ref().unwrap() == &mov {
                            debug!(
                                "backpr from we  l:{} {:4}->{:4}:{:3} ::{}",
                                new_leaf_data.level,
                                leaf_no,
                                adr,
                                new_leaf_data.score,
                                new_leaf_data.score_address
                            );

                            self.backpropagate(
                                states,
                                player, //states[adr].player.clone().as_str(),
                                &adr,
                                &states[adr].clone(),
                            );
                            //}
                        }
                    } else {
                        // opponent
                        // wish lowest score
                        if states[adr].best_move == Some(mov.clone()) {
                            match states[adr].score.cmp(&new_leaf_data.score) {
                                Ordering::Greater => {
                                    states[adr].score = new_leaf_data.score;
                                    states[adr].score_level = new_leaf_data.score_level;
                                    states[adr].score_address = new_leaf_data.score_address;
                                }
                                Ordering::Less => {
                                    let (best_move, best_score, best_score_level, score_address) =
                                        self.get_best_move_and_score(states, &adr);
                                    states[adr].best_move = Some(best_move);
                                    states[adr].score = best_score;
                                    states[adr].score_level = best_score_level;
                                    states[adr].score_address = score_address;
                                }
                                Ordering::Equal => {
                                    // nothing
                                    break 'outer;
                                }
                            }

                            debug!(
                                "backpr opponent l:{} {:4}->{:4}:{:3} ::{}",
                                new_leaf_data.level,
                                leaf_no,
                                adr,
                                new_leaf_data.score,
                                new_leaf_data.score_address
                            );

                            self.backpropagate(
                                states,
                                player, //states[adr].player.clone().as_str(),
                                &adr,
                                &states[adr].clone(),
                            );
                        }
                    }
                    break 'outer;
                }
            }
            adr += 1;
        }
    }

    fn add_leaf(
        &self,
        states: &mut Vec<TSNode>,
        &score: &usize,
        bit_state: &[bool],
        leaf_data: &TSNode,
        is_open: bool,
    ) -> usize {
        // alter player turn in tree
        let new_player = match leaf_data.player == self.me_color {
            false => self.me_color.clone(),
            true => self.opponent_color.clone(),
        };

        states.push(TSNode {
            moves: HashMap::new(),
            score,
            best_move: None,
            bit_state: bit_state.to_owned(),
            is_open,
            level: leaf_data.level + 1,
            player: new_player,
            score_level: leaf_data.level + 1,
            score_address: 1000000,
        });
        states.len() - 1
    }

    /// loop moves and find best move. Look trough move alternatives
    fn get_best_move_and_score(
        &self,
        states: &[TSNode],
        adr: &usize,
    ) -> (String, usize, u8, usize) {
        let parent = states[*adr].clone();
        let mut best_move: String = "".to_string();
        let mut best_score: usize = 0;
        let mut best_score_level = parent.level + 1;
        let mut score_address = 2000000;
        let moves = &parent.moves;
        assert!(
            !moves.is_empty(),
            "Panic if no move alternatives. {:#?}",
            &parent
        );

        for (mov, adr_child) in moves {
            let child_score: usize = states[*adr_child].score;
            let child_score_level: u8 = states[*adr_child].score_level;

            if best_move == *"" {
                best_move = mov.clone();
                best_score = child_score;
                best_score_level = child_score_level;
                score_address = *adr_child;
            } else if parent.player == states[0].player {
                if child_score > best_score || best_score == 0 {
                    best_move = mov.clone();
                    best_score = child_score;
                    best_score_level = child_score_level;
                    score_address = *adr_child;
                }
            } else if child_score < best_score || best_score == 0 {
                best_move = mov.clone();
                best_score = child_score;
                best_score_level = child_score_level;
                score_address = *adr_child;
            }
            if *adr == 0usize && log::log_enabled!(Info) {
                print!(
                    "::{}:{}:{}--",
                    &mov.clone(),
                    adr_child,
                    states[*adr_child].score
                );
            }
        }
        if *adr == 0usize && log::log_enabled!(Info) {
            println!(":{}---", states.len());
        }
        if best_move.is_empty() {
            self.dump_states(states);
            panic!("No best move for: {adr}\n{:#?}", states[*adr]);
        }
        (best_move, best_score, best_score_level, score_address)
    }

    fn dump_state(&self, state: &TSNode) {
        if log::log_enabled!(Info) {
            println!("score,score_level,score_address, is_open,level, player, best_move");

            //print!("{:6}", i);
            print!("{:5}", state.score);
            print!(" {:11}", state.score_level);
            print!(" {:13}", state.score_address);
            print!(" {:8}", state.is_open);
            print!(" {:5}", state.level);
            print!(" {:8}", state.player);
            print!(" {:?}", state.best_move);
            println!(); //  {:?}", states[i].moves);
        }
    }

    fn dump_states(&self, states: &[TSNode]) {
        /*struct TSNode {
            moves: HashMap<String, usize>,
            score: usize,
            best_move: Option<String>,
            bit_state: Vec<bool>,
            is_open: bool,
            level: u8,
            player: String,
            score_level: u8
        }*/
        if log::log_enabled!(Info) {
            println!("address,score,score_level,score_address, is_open,level, player, moves");
            let mut end = states.len();
            if states.len() > 20 {
                end = 10;
            }

            for (i, state) in states.iter().enumerate().take(end) {
                print!("{:7}", i);
                print!(" {:5}", state.score);
                print!(" {:11}", state.score_level);
                print!(" {:13}", state.score_address);
                print!(" {:7}", state.is_open);
                print!(" {:5}", state.level);
                print!(" {:6}", state.player);
                print!(" {:?}", state.best_move);
                println!(); //  {:?}", states[i].moves);
            }
        }
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
    //use rand::rngs::mock;
    // use super::*;
    use crate::player::plgnn_diamond_tree_search::PlayerNNDiamondTS;
    use crate::player::Agentish;
    use boardgame_game::game::connect4::Connect4;
    use boardgame_game::game::Playable;

    #[test]
    fn test_if_able_to_block_opponent_from_winning() {
        //use crate::game::connect4::connect4::new_from_bit_state;
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
            should_be_two,
            "2".to_string(),
            "tree search manage to block opponent"
        );
    }
}
