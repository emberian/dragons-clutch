use clutch_bspline::{BasisSpec, EdgePolicy, MAX_KNOTS, UNIFORM_SPACING_NONE};
use std::io::{self, BufRead};

fn main() {
    for line in io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        let degree: u8 = fields[0].parse().expect("degree");
        let denominator: u64 = fields[1].parse().expect("denominator");
        let value: u128 = fields[2].parse().expect("value");
        let edge_policy = match fields[3] {
            "c" => EdgePolicy::Clamp,
            "r" => EdgePolicy::Refuse,
            _ => panic!("edge"),
        };
        let uniform_log2_spacing = match fields[4] {
            "n" => UNIFORM_SPACING_NONE,
            value => value.parse().expect("spacing"),
        };
        let knot_count: usize = fields[5].parse().expect("knot count");
        assert_eq!(fields.len(), 6 + knot_count);
        let mut knots = [0_u128; MAX_KNOTS];
        for (index, field) in fields[6..].iter().enumerate() {
            knots[index] = field.parse().expect("knot");
        }
        let outcome_count = if degree == 0 {
            knot_count + 1
        } else {
            knot_count - 1 + usize::from(degree)
        };
        let spec = BasisSpec {
            outcome_count: outcome_count as u8,
            degree,
            knot_count: knot_count as u8,
            uniform_log2_spacing,
            denominator,
            domain_max: u128::MAX,
            edge_policy,
            knots,
        };
        match spec.evaluate(value) {
            Ok(vector) => {
                print!("ok");
                for weight in &vector.weights[..outcome_count] {
                    print!(",{weight}");
                }
                println!();
            }
            Err(error) => println!("err,{error:?}"),
        }
    }
}
