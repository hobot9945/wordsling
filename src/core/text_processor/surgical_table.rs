#![allow(unused)]
#[derive(Default)]
pub(super) struct SurgeTable {

    cutting_board: String,

    franken_board: String,

    comb: Vec<Prong>

}

impl SurgeTable {
    pub(super) fn new() -> Self {
        SurgeTable::default()
    }
}

impl Drop for SurgeTable {
    fn drop(&mut self) {
    }
}

#[derive(Default)]
struct Prong {
    cboard_ind: u32,
    fboard_ind: u32,
}