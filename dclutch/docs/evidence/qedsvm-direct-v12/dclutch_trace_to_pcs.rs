//! Convert Mollusk's register trace slots through qedsvm's canonical PcMap.

use qed_analysis::image::ProgramImage;
use std::io::Write as _;

fn main() {
    let mut arguments = std::env::args().skip(1);
    let elf = arguments.next().expect("ELF path");
    let regs =
        std::fs::read(arguments.next().expect("Mollusk .regs path")).expect("register trace");
    let output = arguments.next().expect("output .pcs path");
    assert_eq!(regs.len() % (12 * 8), 0, "complete 12-u64 register rows");
    let image = ProgramImage::load(std::path::Path::new(&elf)).expect("qedsvm program image");
    let mut file = std::fs::File::create(output).expect("output trace");
    for row in regs.chunks_exact(12 * 8) {
        let slot = u64::from_le_bytes(row[88..96].try_into().expect("r11")) as usize;
        let logical = image
            .pc_map
            .slot_to_logical(slot)
            .expect("slot in exact ELF");
        writeln!(file, "{logical}").expect("write logical PC");
    }
}
