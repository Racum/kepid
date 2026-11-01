use std::process::ExitCode;

use kepid::{Elements, KepId, Severity};

const USAGE: &str = "\
usage: kepid <command> <kepid>

commands:
  decode   print the decoded mean elements
  explain  print every field, raw and decoded
  lint     check for physical problems (exit 1 on errors)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, text] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let id: KepId = match text.parse() {
        Ok(id) => id,
        Err(err) => {
            eprintln!("kepid: {err}");
            return ExitCode::from(2);
        }
    };

    match command.as_str() {
        "decode" => match kepid::decode(id) {
            Ok(el) => {
                print_elements(&el);
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("kepid: {err}");
                ExitCode::FAILURE
            }
        },
        "explain" => {
            print!("{}", kepid::explain(id));
            ExitCode::SUCCESS
        }
        "lint" => {
            let findings = kepid::lint(id);
            for finding in &findings {
                println!("{finding}");
            }
            if findings.iter().any(|f| f.severity == Severity::Error) {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn print_elements(el: &Elements) {
    println!("{:>13}: {}", "epoch", el.epoch_iso8601());
    println!("{:>13}: {:.8}", "mean_motion", el.mean_motion);
    println!("{:>13}: {:.7}", "eccentricity", el.eccentricity);
    println!("{:>13}: {:.4}", "inclination", el.inclination);
    println!("{:>13}: {:.4}", "raan", el.raan);
    println!("{:>13}: {:.4}", "arg_perigee", el.arg_perigee);
    println!("{:>13}: {:.4}", "mean_anomaly", el.mean_anomaly);
    println!("{:>13}: {:.4e}", "bstar", el.bstar);
}
