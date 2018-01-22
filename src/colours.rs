use SourceLoc;

use ansi_term::{Style, Colour};

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

/// This is a random sequence which was generated. It is used to determine which
/// order to display colours in when drawing the colourized output.
///
/// XXX(nika): Try to improve this in the future.
const COLOUR_SEQUENCE: &[u8] = &[
    // Try to use the system colours if they are avaliable.
    1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14, 15,

    // Randomly shuffle through the remaining colours after that point.
    34, 185, 147, 79, 214, 216, 152, 22, 63, 56, 192, 73, 110, 148, 136, 43,
    109, 221, 179, 111, 105, 115, 211, 20, 155, 166, 172, 222, 206, 85, 231,
    124, 108, 118, 65, 114, 83, 19, 78, 187, 98, 42, 157, 21, 32, 210, 48, 53,
    229, 160, 138, 142, 219, 220, 66, 145, 154, 35, 106, 133, 75, 191, 176,
    169, 230, 125, 149, 103, 96, 190, 74, 150, 228, 174, 204, 193, 92, 31,
    208, 181, 94, 197, 89, 223, 139, 27, 202, 141, 213, 45, 194, 218, 77, 68,
    126, 189, 70, 23, 121, 93, 183, 132, 52, 87, 44, 116, 49, 225, 119, 61, 76,
    135, 161, 46, 163, 104, 140, 67, 97, 81, 64, 50, 180, 217, 178, 165, 37,
    215, 99, 186, 171, 86, 57, 137, 41, 47, 153, 201, 173, 170, 29, 88, 128,
    175, 182, 226, 184, 102, 24, 195, 36, 168, 60, 30, 38, 26, 159, 58, 51,
    199, 91, 54, 71, 101, 143, 144, 203, 120, 39, 167, 59, 158, 62, 100, 130,
    82, 112, 123, 162, 205, 117, 207, 25, 209, 156, 84, 113, 224, 200, 33, 134,
    198, 188, 164, 212, 146, 122, 80, 177, 131, 227, 72, 16, 151, 196, 90, 17,
    129, 28, 55, 107, 95, 127, 18, 40, 69,
];

fn map_to_colour(i: usize) -> Style {
    let i = COLOUR_SEQUENCE[i % COLOUR_SEQUENCE.len()];
    if i < 16 {
        // XXX(nika): Figure out what colour to use here?
        return Style::new().on(Colour::Fixed(i)).fg(Colour::Black);
    }

    let row_idx = (i - 16) % 36;
    let mut style = Style::new().on(Colour::Fixed(i));
    if row_idx < 18 {
        style = style.fg(Colour::White);
    } else {
        style = style.fg(Colour::Black);
    }
    style
}

static CURRENT_COLOUR: AtomicUsize = ATOMIC_USIZE_INIT;
impl SourceLoc {
    pub(crate) fn style(&self) -> Style {
        let idx = self.colour.load(Ordering::SeqCst);
        if idx == 0 {
            let idx = CURRENT_COLOUR.fetch_add(1, Ordering::SeqCst);
            self.colour.compare_and_swap(0, idx + 1, Ordering::SeqCst);
            return self.style();
        }
        map_to_colour(idx - 1)
    }
}

#[test]
fn colour_test() {
    // This test doesn't actually assert anything, but rather just is used to
    // visualize all colours by turning off output capturing.
    for i in 0..COLOUR_SEQUENCE.len() {
        println!("{}", map_to_colour(i).paint("Hello, World!"));
    }
}
