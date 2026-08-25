//! Finite Lean-to-Rust differential validator for Product payoffs.

use std::{collections::BTreeMap, env, error::Error, fmt, fs, path::Path};

use dclutch_product_payoff_codec::{ABI_BYTES, ProductPayoff};

#[derive(Debug)]
struct Failure(String);

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for Failure {}

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Statistics {
    programs: usize,
    mutations: usize,
    hostile_widths: usize,
    evaluations: usize,
    evaluation_accepts: usize,
    evaluation_refusals: usize,
    collateral: usize,
    hostile_programs: usize,
}

fn failure(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(Failure(message.into()))
}

fn require_equal<T: PartialEq + fmt::Debug>(actual: T, expected: T, context: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(failure(format!(
            "{context}: mismatch\n  actual: {actual:?}\nexpected: {expected:?}"
        )))
    }
}

fn parse_u64(value: &str, context: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .map_err(|error| failure(format!("{context}: invalid u64 {value:?}: {error}")))
}

fn parse_usize(value: &str, context: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .map_err(|error| failure(format!("{context}: invalid usize {value:?}: {error}")))
}

fn field<'a>(fields: &'a [&str], index: usize, context: &str) -> Result<&'a str> {
    fields
        .get(index)
        .copied()
        .ok_or_else(|| failure(format!("{context}: missing field {index}")))
}

fn disposition(value: &str, context: &str) -> Result<bool> {
    match value {
        "accept" => Ok(true),
        "reject" => Ok(false),
        _ => Err(failure(format!("{context}: invalid disposition {value:?}"))),
    }
}

fn nibble(byte: u8, context: &str) -> Result<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(failure(format!(
            "{context}: noncanonical hex byte {byte:?}"
        ))),
    }
}

fn decode_hex(value: &str, context: &str) -> Result<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(failure(format!("{context}: odd hex width")));
    }
    bytes
        .chunks_exact(2)
        .map(|pair| {
            let high = pair
                .first()
                .copied()
                .ok_or_else(|| failure(format!("{context}: missing high nibble")))?;
            let low = pair
                .get(1)
                .copied()
                .ok_or_else(|| failure(format!("{context}: missing low nibble")))?;
            Ok((nibble(high, context)? << 4) | nibble(low, context)?)
        })
        .collect()
}

fn validate(path: &Path) -> Result<Statistics> {
    let corpus = fs::read_to_string(path)?;
    let mut programs = BTreeMap::<String, Vec<u8>>::new();
    let mut statistics = Statistics::default();

    for (line_index, line) in corpus.lines().enumerate() {
        let fields: Vec<_> = line.split('|').collect();
        let line_number = line_index + 1;
        match fields.first().copied() {
            Some("payoff") => {
                if fields.len() != 4 {
                    return Err(failure(format!("line {line_number}: payoff field count")));
                }
                let name = field(&fields, 1, "payoff")?;
                let context = format!("payoff {name}");
                let bytes = decode_hex(field(&fields, 2, &context)?, &context)?;
                require_equal(bytes.len(), ABI_BYTES, &context)?;
                let program = ProductPayoff::decode(&bytes)
                    .map_err(|error| failure(format!("{context}: Rust refused: {error:?}")))?;
                require_equal(program.encode().as_slice(), bytes.as_slice(), &context)?;
                require_equal(
                    program.liability_bound(),
                    parse_u64(field(&fields, 3, &context)?, &context)?,
                    &format!("{context}: liability"),
                )?;
                if programs.insert(name.to_owned(), bytes).is_some() {
                    return Err(failure(format!("{context}: duplicate name")));
                }
                statistics.programs += 1;
            }
            Some("payoff-mutation") => {
                if fields.len() != 4 {
                    return Err(failure(format!("line {line_number}: mutation field count")));
                }
                let name = field(&fields, 1, "payoff mutation")?;
                let offset_field = field(&fields, 2, "payoff mutation")?;
                let context = format!("payoff mutation {name} byte {offset_field}");
                let mut bytes = programs
                    .get(name)
                    .cloned()
                    .ok_or_else(|| failure(format!("{context}: unknown program")))?;
                let offset = parse_usize(offset_field, &context)?;
                let byte = bytes
                    .get_mut(offset)
                    .ok_or_else(|| failure(format!("{context}: offset out of bounds")))?;
                *byte = byte.wrapping_add(1);
                require_equal(
                    ProductPayoff::decode(&bytes).is_ok(),
                    disposition(field(&fields, 3, &context)?, &context)?,
                    &context,
                )?;
                statistics.mutations += 1;
            }
            Some("payoff-hostile-width") => {
                if fields.len() != 5 {
                    return Err(failure(format!("line {line_number}: width field count")));
                }
                let name = field(&fields, 1, "payoff hostile width")?;
                let label = field(&fields, 2, "payoff hostile width")?;
                let context = format!("payoff hostile width {name} {label}");
                if !programs.contains_key(name) {
                    return Err(failure(format!("{context}: unknown program")));
                }
                let bytes = decode_hex(field(&fields, 3, &context)?, &context)?;
                require_equal(
                    ProductPayoff::decode(&bytes).is_ok(),
                    disposition(field(&fields, 4, &context)?, &context)?,
                    &context,
                )?;
                statistics.hostile_widths += 1;
            }
            Some("payoff-eval") => {
                if fields.len() != 4 && fields.len() != 5 {
                    return Err(failure(format!("line {line_number}: eval field count")));
                }
                let name = field(&fields, 1, "payoff eval")?;
                let coordinate_field = field(&fields, 2, "payoff eval")?;
                let context = format!("payoff eval {name} coordinate {coordinate_field}");
                let bytes = programs
                    .get(name)
                    .ok_or_else(|| failure(format!("{context}: unknown program")))?;
                let program = ProductPayoff::decode(bytes)
                    .map_err(|error| failure(format!("{context}: canonical decode: {error:?}")))?;
                let result = program.evaluate(parse_u64(coordinate_field, &context)?);
                let expected_accept = disposition(field(&fields, 3, &context)?, &context)?;
                require_equal(result.is_ok(), expected_accept, &context)?;
                if expected_accept {
                    if fields.len() != 5 {
                        return Err(failure(format!("{context}: result omitted")));
                    }
                    require_equal(
                        result,
                        Ok(parse_u64(field(&fields, 4, &context)?, &context)?),
                        &context,
                    )?;
                    statistics.evaluation_accepts += 1;
                } else {
                    if fields.len() != 4 {
                        return Err(failure(format!("{context}: refusal carried result")));
                    }
                    statistics.evaluation_refusals += 1;
                }
                statistics.evaluations += 1;
            }
            Some("payoff-collateral") => {
                if fields.len() != 4 {
                    return Err(failure(format!(
                        "line {line_number}: collateral field count"
                    )));
                }
                let name = field(&fields, 1, "payoff collateral")?;
                let amount = field(&fields, 2, "payoff collateral")?;
                let context = format!("payoff collateral {name} amount {amount}");
                let bytes = programs
                    .get(name)
                    .ok_or_else(|| failure(format!("{context}: unknown program")))?;
                let program = ProductPayoff::decode(bytes)
                    .map_err(|error| failure(format!("{context}: canonical decode: {error:?}")))?;
                require_equal(
                    program.collateralized_by(parse_u64(amount, &context)?),
                    disposition(field(&fields, 3, &context)?, &context)?,
                    &context,
                )?;
                statistics.collateral += 1;
            }
            Some("payoff-hostile") => {
                if fields.len() != 4 {
                    return Err(failure(format!("line {line_number}: hostile field count")));
                }
                let name = field(&fields, 1, "payoff hostile")?;
                let context = format!("payoff hostile {name}");
                let bytes = decode_hex(field(&fields, 2, &context)?, &context)?;
                require_equal(
                    ProductPayoff::decode(&bytes).is_ok(),
                    disposition(field(&fields, 3, &context)?, &context)?,
                    &context,
                )?;
                statistics.hostile_programs += 1;
            }
            Some(kind) => {
                return Err(failure(format!(
                    "line {line_number}: unknown record {kind:?}"
                )));
            }
            None => return Err(failure(format!("line {line_number}: empty record"))),
        }
    }

    require_equal(
        (
            statistics.programs,
            statistics.mutations,
            statistics.hostile_widths,
            statistics.evaluations,
            statistics.evaluation_accepts,
            statistics.evaluation_refusals,
            statistics.collateral,
            statistics.hostile_programs,
        ),
        (6, 2_592, 2_598, 38, 33, 5, 18, 23),
        "corpus accounting",
    )?;
    Ok(statistics)
}

fn main() -> Result<()> {
    let mut arguments = env::args_os();
    let executable = arguments.next().unwrap_or_default();
    let path = arguments.next().ok_or_else(|| {
        failure(format!(
            "usage: {} CORPUS",
            Path::new(&executable).display()
        ))
    })?;
    if arguments.next().is_some() {
        return Err(failure("expected exactly one corpus path"));
    }
    let statistics = validate(Path::new(&path))?;
    println!(
        "Product payoff translation passed: {} canonical ABIs, {} byte mutations, {} hostile widths, {} evaluations ({} accepted/{} refused), {} collateral checks, {} structural hostiles",
        statistics.programs,
        statistics.mutations,
        statistics.hostile_widths,
        statistics.evaluations,
        statistics.evaluation_accepts,
        statistics.evaluation_refusals,
        statistics.collateral,
        statistics.hostile_programs,
    );
    Ok(())
}
