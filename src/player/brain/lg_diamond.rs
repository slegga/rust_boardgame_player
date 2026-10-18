///BitOp logic_gates er mitt første forsøk på å lage nevralt nettverk
/// Gjør det enkelt.
/// Hver nevron er en bit
/// bit 0 = false
/// bit 1 = true
/// bit 2 = Er det betrakters sin tur
/// bit 3 = Hvilken farge har betrakter false = første spiller, true = andre spiller
/// bit 4 = Spill ferdig. Uavgjort.
/// bit 5 = Betrakter vant
/// bit 6 = Betrakter tapte
/// bit 7+++ Spillbrett
///
/// Layer begynner med perseptio
///
/// layer0 [cell0]..[32]
/// layer1 [cell0][1][2][3]..[16]
/// layer2 [cell0][1][2][3][4][5][6][7]
/// layer3 [cell0][1][2][3]
/// layer4 [cell0][cell1]
/// layer5 [cell: gate:0..255]
/// gate(type,address to cell0,address to cell1)
// use env_logger;
use log::{debug, trace};
use rand::Rng;
//use std::char;
use std::fs::File;

use crate::player::brain::*;
use boardgame_game::game::GameStatic;
//use GameStatic;
use std::io::Read;
use std::io::Write;

/// Layer representerer et lag med nevroner
#[derive(Clone, Debug)]
pub struct BrainDiamond {
    pub game_name: String,
    pub filepath: String,
    pub layers: [Vec<Cell>; NO_LAYERS],
}

impl Default for BrainDiamond {
    fn default() -> Self {
        let mut layers = vec![];
        for l in 0..NO_LAYERS {
            layers.push(get_default_cell_layer(l.try_into().unwrap()));
        }
        BrainDiamond {
            game_name: "Not set".to_string(),
            filepath: "".to_string(),
            layers: layers.try_into().unwrap(),
        }
    }
}
impl BrainDiamond {
    /// main function. Evaluate bit_state
    /// bit_state har false,true,aktiveller ei,farge,tomme,mine og oponents brikker
    pub fn evaluate_bit_state(
        &self,
        //       game_static: GameStatic,
        //       current_player: &str,
        //       current_player_color: &str,
        state: &Vec<bool>,
    ) -> usize {
        //     let is_current_active_player = state[0] ^ (current_player == "A");

        //let mut current_bit_state = vec![false, true, is_current_active_player];
        let mut current_bit_state = state.clone();
        // println!("in:{:?}", current_state);
        let two: usize = 2;

        // If game is terminated, return hardcoded values
        // bit 4,5,6, is for terminal states. So evalueate them. If not set continiue

        if state[4] {
            //Draw
            return CELL_SIZE / 2;
        } else if state[5] {
            //Me is the winner
            return CELL_SIZE;
        } else if state[6] {
            //opponent is the winner
            return 0;
        }

        for layer_no in 0..NO_LAYERS {
            let layer: &Vec<Cell> = &self.layers[layer_no];
            let cells: usize = two.pow((NO_LAYERS - layer_no - 1) as u32);
            let mut next_bit_state: Vec<bool> = vec![true; CELL_SIZE * cells];
            /*println!(
                "evaluate_bit_state: {} {} current_bit_state.len {}",
                layer_no,
                cells,
                current_bit_state.len()
            );*/
            if layer_no == 0 {
                for cell in 0..cells {
                    for gate in 0..CELL_SIZE {
                        let bit_a = current_bit_state[layer[cell].address_a[gate]];
                        let bit_b = current_bit_state[layer[cell].address_b[gate]];
                        next_bit_state[cell * CELL_SIZE + gate] = match layer[cell].operator[gate] {
                            BitOp::FALSE => false, //         0 False 0                       0  0  0  0
                            BitOp::AND => bit_a & bit_b, // 1 A ∧ B A · B                   0  0  0  1
                            BitOp::ANDANB => bit_a & !bit_b, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                            BitOp::A => bit_a, // 3 A A                           0  0  1  1
                            BitOp::ANDNAB => !bit_a & bit_b, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                            BitOp::B => bit_b, // 5 B B                           0  1  0  1
                            BitOp::XOR => bit_a ^ bit_b, // 6 A ⊕ B A + B − 2AB             0  1  1  0
                            BitOp::OR => bit_a | bit_b, // 7 A ∨ B A + B − AB              0  1  1  1
                            BitOp::NOR => !(bit_a | bit_b), // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                            BitOp::NXOR => !(bit_a ^ bit_b), // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                            BitOp::NB => !bit_b, // 10 ¬B 1 − B                     1  0  1  0
                            BitOp::ORANB => bit_a | !bit_b, //11 A ⇐ B 1 − B + AB             1  0  1  1
                            BitOp::NA => !bit_a, //12 ¬A 1 − A                     1  1  0  0
                            BitOp::ORNAB => !bit_a | bit_b, //13 A ⇒ B 1 − A + AB             1  1  0  1
                            BitOp::NAND => !(bit_a & bit_b), //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                            BitOp::TRUE => true, // 15 True 1                       1  1  1  1
                        };
                    }
                }
            } else {
                for cell in 0..cells {
                    for gate in 0..CELL_SIZE {
                        let bit_a = current_bit_state[cell * 2 * CELL_SIZE + gate];
                        let bit_b = current_bit_state[(cell * 2 + 1) * CELL_SIZE + gate];
                        next_bit_state[cell * CELL_SIZE + gate] = match layer[cell].operator[gate] {
                            BitOp::FALSE => false, //         0 False 0                       0  0  0  0
                            BitOp::AND => bit_a & bit_b, // 1 A ∧ B A · B                   0  0  0  1
                            BitOp::ANDANB => bit_a & !bit_b, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                            BitOp::A => bit_a, // 3 A A                           0  0  1  1
                            BitOp::ANDNAB => !bit_a & bit_b, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                            BitOp::B => bit_b, // 5 B B                           0  1  0  1
                            BitOp::XOR => bit_a ^ bit_b, // 6 A ⊕ B A + B − 2AB             0  1  1  0
                            BitOp::OR => bit_a | bit_b, // 7 A ∨ B A + B − AB              0  1  1  1
                            BitOp::NOR => !(bit_a | bit_b), // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                            BitOp::NXOR => !(bit_a ^ bit_b), // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                            BitOp::NB => !bit_b, // 10 ¬B 1 − B                     1  0  1  0
                            BitOp::ORANB => bit_a | !bit_b, //11 A ⇐ B 1 − B + AB             1  0  1  1
                            BitOp::NA => !bit_a, //12 ¬A 1 − A                     1  1  0  0
                            BitOp::ORNAB => !bit_a | bit_b, //13 A ⇒ B 1 − A + AB             1  1  0  1
                            BitOp::NAND => !(bit_a & bit_b), //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                            BitOp::TRUE => true, // 15 True 1                       1  1  1  1
                        };
                    }
                }
            }
            // println!("{}: {:?}", layer_no, next_state);
            current_bit_state = next_bit_state;
        }
        // return number of true bits
        current_bit_state.iter().filter(|&x| *x).count()
    }

    pub fn from_file(&mut self) -> Result<(), LogicGatesError> {
        if self.filepath.is_empty() {
            self.filepath = format!(
                "data/{}-lgnndiamond-{}-{}.txt",
                self.game_name, NO_LAYERS, CELL_SIZE
            );
        }
        // warn!("Filename: {}", &self.filepath);
        let mut logic_gates_nn: File = File::open(&self.filepath)?;
        let mut logic_gates_nn_string = String::new(); // This String will hold it
        logic_gates_nn.read_to_string(&mut logic_gates_nn_string)?; // Read the file into it

        //logic_gates_nn_string.split_ split_whitespace().for_each(|word| println!("{} ", word.to_uppercase())); // Do things with the String now

        // For å få kjørt kjør: rm data/logic_gate_nn.txt
        let mut layer_no = 0;
        let mut cell_no = 0;
        let mut gate_no: usize = 0;
        for line in logic_gates_nn_string.lines() {
            let line_string = line.to_string();
            if line_string.starts_with("Layer:") {
                gate_no = 0;
                cell_no = 0;
                let no_str = line.rsplit_once(" ").unwrap_or(("", "")).1;
                layer_no = no_str
                    .to_string()
                    .parse::<usize>()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                // println!("from_file: {} layer_no {}", &self.filepath, layer_no);
            } else if line_string.eq("Data:") {
                // do nothing
            } else {
                match line_string.chars().next() {
                    Some(char) => {
                        if char.is_numeric() {
                            let mut gate_iter = line_string.split(",");
                            let adr_a: usize = gate_iter
                                .next()
                                .unwrap_or("")
                                .parse::<usize>()
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                            let adr_b: usize = gate_iter
                                .next()
                                .unwrap_or("")
                                .parse::<usize>()
                                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                            let operator =
                                gate_iter.next().unwrap_or("XOR").parse::<BitOp>().unwrap(); // forstår ikke hvordan bli kvitt unwrap her. (Fikk feil med expect også.)

                            self.layers[layer_no][cell_no].address_a[gate_no] = adr_a;
                            self.layers[layer_no][cell_no].address_b[gate_no] = adr_b;
                            self.layers[layer_no][cell_no].operator[gate_no] = operator;
                        } else {
                            return Err(LogicGatesError::InvalidChar {
                                char,
                                line_string,
                                file: self.filepath.clone(),
                            });
                        }
                    }
                    None => {
                        panic!("Nothing Handle {layer_no}:{gate_no} for {line_string}")
                    }
                }
                trace!("Handle {layer_no}:{gate_no} for {line_string}");
                gate_no += 1;
                if gate_no >= CELL_SIZE {
                    gate_no -= CELL_SIZE;
                    cell_no += 1;
                }
            }
        }
        Ok(())
    }

    pub fn save_to_file(&self) -> Result<(), LogicGatesError> {
        {
            debug!("File to load: {}", self.filepath);

            if self.filepath.is_empty() {
                return Err(LogicGatesError::General {
                    message: "Missing filepath".to_string(),
                });
            }
            // truncate file
            let _ = File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&self.filepath)?;
            {
                //  writeln!(&mut f, "")?;
            }
            //
        }
        let mut f = File::options()
            .create(true)
            .append(true)
            .open(&self.filepath)?;
        debug!("INSIDE SAVE_TO_FILE");
        for layer in 0..NO_LAYERS {
            writeln!(&mut f, "Layer: {}", layer)?;
            writeln!(&mut f, "Data:")?;
            /*println!(
                "save_to_file layer{} - cells:{}",
                layer,
                usize::pow(2, (NO_LAYERS - layer - 1) as u32)
            );*/
            // for i in (0..NO_LAYERS).rev().map(|x| usize::pow(x,2)
            for i in 0..usize::pow(2, NO_LAYERS as u32 - layer as u32 - 1u32) {
                for j in 0..CELL_SIZE {
                    writeln!(
                        &mut f,
                        "{},{},{:?}",
                        self.layers[layer][i].address_a[j],
                        self.layers[layer][i].address_b[j],
                        self.layers[layer][i].operator[j]
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Change random gates. Both type and input addresses.
    /// Do this 1% of gates.
    pub fn do_mutate(&mut self, max_address: &usize) {
        let mut rng: rand::prelude::ThreadRng = rand::thread_rng();
        let two: u32 = 2;
        let num_tot_cells = two.pow(NO_LAYERS as u32) - 1 - 1;
        let total_gates = num_tot_cells * CELL_SIZE as u32;
        for _i in 0..total_gates / 100 {
            let mut cell = rng.gen_range(0..num_tot_cells) as usize;

            // find cell address (layer,cell)
            let mut layer = NO_LAYERS - 1;
            for j in 0..NO_LAYERS as u32 {
                let cells_in_layer = usize::pow(2, NO_LAYERS as u32 - j - 1);
                if cell >= cells_in_layer {
                    layer -= 1;
                    cell -= cells_in_layer;
                }
            }

            //let layer = rng.gen_range(0..NO_LAYERS);
            let gate_address_a = match layer {
                0 => rng.gen_range(0..*max_address),
                _ => 0,
            };
            let gate_address_b = match layer {
                0 => rng.gen_range(0..*max_address),
                _ => 0,
            };
            let gate_id = rng.gen_range(0..CELL_SIZE);
            if layer == 0 {
                self.layers[layer][cell].address_a[gate_id] = gate_address_a;
                self.layers[layer][cell].address_b[gate_id] = gate_address_b;
            } else {
                self.layers[layer][cell].address_a[gate_id] = gate_id;
                self.layers[layer][cell].address_b[gate_id] = gate_id;
            }
            if self.layers[layer].len() <= cell {
                panic!("self.layers[layer].len()> cell. Should be {} > {}. gate_id = {}, num_tot_cells={}, layer={}",
                    self.layers[layer].len(), cell, gate_id,num_tot_cells,layer );
            }
            self.layers[layer][cell as usize].operator[gate_id] = match rng.gen_range(0..16) {
                0 => BitOp::FALSE,  //         0 False 0                       0  0  0  0
                1 => BitOp::AND,    // 1 A ∧ B A · B                   0  0  0  1
                2 => BitOp::ANDANB, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                3 => BitOp::A,      // 3 A A                           0  0  1  1
                4 => BitOp::ANDNAB, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                5 => BitOp::B,      // 5 B B                           0  1  0  1
                6 => BitOp::XOR,    // 6 A ⊕ B A + B − 2AB             0  1  1  0
                7 => BitOp::OR,     // 7 A ∨ B A + B − AB              0  1  1  1
                8 => BitOp::NOR,    // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                9 => BitOp::NXOR,   // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                10 => BitOp::NB,    // 10 ¬B 1 − B                     1  0  1  0
                11 => BitOp::ORANB, //11 A ⇐ B 1 − B + AB             1  0  1  1
                12 => BitOp::NA,    //12 ¬A 1 − A                     1  1  0  0
                13 => BitOp::ORNAB, //13 A ⇒ B 1 − A + AB             1  1  0  1
                14 => BitOp::NAND,  //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                15 => BitOp::TRUE,  // 15 True 1                       1  1  1  1

                x => panic!("gen_range out of range: 0..6 {x}"),
            };
        }
    }


    /// Returnerer bit_state før angitt level. Så level 0 = game_bit_state, NO_LAYERS = siste resultat som er grunnlaget for summeringen summering.
    pub fn get_gate_result_for_level(&self,game_bit_state: Vec<bool>, level: u8)->Vec<bool> {
        //todo!("get_gate_result_for_level. Returner alle bit før angitt level");
        //let mut current_bit_state = vec![false, true, is_current_active_player];
        let mut current_bit_state = game_bit_state.clone();
        // println!("in:{:?}", current_state);
        let two: usize = 2;

        // If game is terminated, return hardcoded values
        // bit 4,5,6, is for terminal states. So evalueate them. If not set continiue


        for layer_no in 0..level as usize {
            let layer: &Vec<Cell> = &self.layers[layer_no];
            let cells: usize = two.pow((NO_LAYERS - layer_no - 1) as u32);
            let mut next_bit_state: Vec<bool> = vec![true; CELL_SIZE * cells];
            /*println!(
                "evaluate_bit_state: {} {} current_bit_state.len {}",
                layer_no,
                cells,
                current_bit_state.len()
            );*/
            if layer_no == 0 {
                for cell in 0..cells {
                    for gate in 0..CELL_SIZE {
                        let bit_a = current_bit_state[layer[cell].address_a[gate]];
                        let bit_b = current_bit_state[layer[cell].address_b[gate]];
                        next_bit_state[cell * CELL_SIZE + gate] = match layer[cell].operator[gate] {
                            BitOp::FALSE => false, //         0 False 0                       0  0  0  0
                            BitOp::AND => bit_a & bit_b, // 1 A ∧ B A · B                   0  0  0  1
                            BitOp::ANDANB => bit_a & !bit_b, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                            BitOp::A => bit_a, // 3 A A                           0  0  1  1
                            BitOp::ANDNAB => !bit_a & bit_b, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                            BitOp::B => bit_b, // 5 B B                           0  1  0  1
                            BitOp::XOR => bit_a ^ bit_b, // 6 A ⊕ B A + B − 2AB             0  1  1  0
                            BitOp::OR => bit_a | bit_b, // 7 A ∨ B A + B − AB              0  1  1  1
                            BitOp::NOR => !(bit_a | bit_b), // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                            BitOp::NXOR => !(bit_a ^ bit_b), // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                            BitOp::NB => !bit_b, // 10 ¬B 1 − B                     1  0  1  0
                            BitOp::ORANB => bit_a | !bit_b, //11 A ⇐ B 1 − B + AB             1  0  1  1
                            BitOp::NA => !bit_a, //12 ¬A 1 − A                     1  1  0  0
                            BitOp::ORNAB => !bit_a | bit_b, //13 A ⇒ B 1 − A + AB             1  1  0  1
                            BitOp::NAND => !(bit_a & bit_b), //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                            BitOp::TRUE => true, // 15 True 1                       1  1  1  1
                        };
                    }
                }
            } else {
                for cell in 0..cells {
                    for gate in 0..CELL_SIZE {
                        let bit_a = current_bit_state[cell * 2 * CELL_SIZE + gate];
                        let bit_b = current_bit_state[(cell * 2 + 1) * CELL_SIZE + gate];
                        next_bit_state[cell * CELL_SIZE + gate] = match layer[cell].operator[gate] {
                            BitOp::FALSE => false, //         0 False 0                       0  0  0  0
                            BitOp::AND => bit_a & bit_b, // 1 A ∧ B A · B                   0  0  0  1
                            BitOp::ANDANB => bit_a & !bit_b, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                            BitOp::A => bit_a, // 3 A A                           0  0  1  1
                            BitOp::ANDNAB => !bit_a & bit_b, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                            BitOp::B => bit_b, // 5 B B                           0  1  0  1
                            BitOp::XOR => bit_a ^ bit_b, // 6 A ⊕ B A + B − 2AB             0  1  1  0
                            BitOp::OR => bit_a | bit_b, // 7 A ∨ B A + B − AB              0  1  1  1
                            BitOp::NOR => !(bit_a | bit_b), // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                            BitOp::NXOR => !(bit_a ^ bit_b), // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                            BitOp::NB => !bit_b, // 10 ¬B 1 − B                     1  0  1  0
                            BitOp::ORANB => bit_a | !bit_b, //11 A ⇐ B 1 − B + AB             1  0  1  1
                            BitOp::NA => !bit_a, //12 ¬A 1 − A                     1  1  0  0
                            BitOp::ORNAB => !bit_a | bit_b, //13 A ⇒ B 1 − A + AB             1  1  0  1
                            BitOp::NAND => !(bit_a & bit_b), //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                            BitOp::TRUE => true, // 15 True 1                       1  1  1  1
                        };
                    }
                }
            }
            // println!("{}: {:?}", layer_no, next_state);
            current_bit_state = next_bit_state;
        }
        // return number of true bits
        current_bit_state

    }
}

/// Layer representerer et lag med nevroner
/// "cell layer" En "cell" er 256 gates.
/// "layer" er 2**(Layers - layer -1) antall "cell"
/// "Layer" i denne sammenheng er en Vec
pub fn get_default_cell_layer(layer: u32) -> Vec<Cell> {
    let base: usize = 2;
    let redo = base.pow(5 - layer);
    let mut retur = vec![];

    // Bygger default_cell
    let mut address_default: [usize; CELL_SIZE] = [0; CELL_SIZE];
    if layer > 0 {
        for i in 0..CELL_SIZE {
            address_default[i] = i;
        }
    }
    let default_cell: Cell = Cell {
        address_a: address_default,
        address_b: address_default,
        operator: [BitOp::AND; CELL_SIZE],
    };

    // Bygger default_layer
    retur.resize(redo, default_cell);
    retur
}

/// This function differ from
fn generate_random_cell_layer(max_address: &usize, layer: u32) -> Vec<Cell> {
    let mut retur: Vec<Cell> = get_default_cell_layer(layer);
    let mut rng: rand::prelude::ThreadRng = rand::thread_rng();
    for cell_layer in &mut retur {
        for i in 0..CELL_SIZE {
            // layer 0 har tilfeldige adresser for a og b, mens resten er satt.
            match layer {
                0 => {
                    cell_layer.address_a[i] = rng.gen_range(0..*max_address);
                    cell_layer.address_b[i] = rng.gen_range(0..*max_address);
                    cell_layer.operator[i] = match rng.gen_range(1..15) {
                        // rand 0.8
                        0 => BitOp::FALSE, //         0 False 0                       0  0  0  0
                        1 => BitOp::AND,   // 1 A ∧ B A · B                   0  0  0  1
                        2 => BitOp::ANDANB, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                        3 => BitOp::A,     // 3 A A                           0  0  1  1
                        4 => BitOp::ANDNAB, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                        5 => BitOp::B,     // 5 B B                           0  1  0  1
                        6 => BitOp::XOR,   // 6 A ⊕ B A + B − 2AB             0  1  1  0
                        7 => BitOp::OR,    // 7 A ∨ B A + B − AB              0  1  1  1
                        8 => BitOp::NOR,   // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                        9 => BitOp::NXOR,  // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                        10 => BitOp::NB,   // 10 ¬B 1 − B                     1  0  1  0
                        11 => BitOp::ORANB, //11 A ⇐ B 1 − B + AB             1  0  1  1
                        12 => BitOp::NA,   //12 ¬A 1 − A                     1  1  0  0
                        13 => BitOp::ORNAB, //13 A ⇒ B 1 − A + AB             1  1  0  1
                        14 => BitOp::NAND, //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                        15 => BitOp::TRUE, // 15 True 1                       1  1  1  1
                        x => panic!("gen_range out of range: 0..6 {x}"),
                    };
                }
                _ => {
                    cell_layer.address_a[i] = i;
                    cell_layer.address_b[i] = i;
                    cell_layer.operator[i] = match rng.gen_range(1..15) {
                        // rand 0.8
                        0 => BitOp::FALSE, //         0 False 0                       0  0  0  0
                        1 => BitOp::AND,   // 1 A ∧ B A · B                   0  0  0  1
                        2 => BitOp::ANDANB, // 2 ¬(A ⇒ B) A − AB               0  0  1  0
                        3 => BitOp::A,     // 3 A A                           0  0  1  1
                        4 => BitOp::ANDNAB, // 4 ¬(A ⇐ B) B − AB               0  1  0  0
                        5 => BitOp::B,     // 5 B B                           0  1  0  1
                        6 => BitOp::XOR,   // 6 A ⊕ B A + B − 2AB             0  1  1  0
                        7 => BitOp::OR,    // 7 A ∨ B A + B − AB              0  1  1  1
                        8 => BitOp::NOR,   // 8 ¬(A ∨ B) 1 − (A + B − AB)     1  0  0  0
                        9 => BitOp::NXOR,  // 9 ¬(A ⊕ B) 1 − (A + B − 2AB)    1  0  0  1
                        10 => BitOp::NB,   // 10 ¬B 1 − B                     1  0  1  0
                        11 => BitOp::ORANB, //11 A ⇐ B 1 − B + AB             1  0  1  1
                        12 => BitOp::NA,   //12 ¬A 1 − A                     1  1  0  0
                        13 => BitOp::ORNAB, //13 A ⇒ B 1 − A + AB             1  1  0  1
                        14 => BitOp::NAND, //14 ¬(A ∧ B) 1 − AB              1  1  1  0
                        15 => BitOp::TRUE, // 15 True 1                       1  1  1  1
                        x => panic!("gen_range out of range: 0..6 {x}"),
                    };
                }
            };
        }
    }
    //println!("{:?}", retur);

    retur
}
pub fn generate_random_brain(game: &GameStatic) -> BrainDiamond {
    //game: crate::game::game::GameStatic
    let mut layers = vec![];
    for l in 0..NO_LAYERS {
        layers.push(get_default_cell_layer(l.try_into().unwrap()));
    }
    let mut brain = BrainDiamond {
        game_name: game.name.clone(),
        filepath: format!(
            "data/{}-lgnndiamond-{}-{}.txt",
            game.name, NO_LAYERS, CELL_SIZE
        ),
        layers: layers.try_into().unwrap(),
    };
    for i in 0..NO_LAYERS {
        if i == 0 {
            let state_size = game.get_state_size();
            brain.layers[i] = generate_random_cell_layer(&state_size, i as u32);
        } else {
            brain.layers[i] = generate_random_cell_layer(&CELL_SIZE, i as u32);
        }
    }

    brain
}
