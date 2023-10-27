use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport as V2Transport;
use ic_agent::{agent::AgentConfig, export::Principal, Agent};

use candid::encode_args;
use candid::{CandidType, Decode, Nat};

use std::io::Write;

use byte_unit::Byte;

use humantime::Timestamp;
use std::ops::Add;
use std::time::SystemTime;
use structopt::StructOpt;

mod movm;
use core::hash::Hasher;
use motoko::ast::Inst;
use motoko::dynamic::{Dynamic, Result as MoRes};
use motoko::vm_types::Store;
use motoko::Share;
use motoko::{Value, Value_};
use std::hash::Hash;

use std::fs::File;

use serde::{Deserialize, Serialize};
use std::io::BufReader;

#[derive(Debug, Deserialize)]
struct CanisterIds {
    // to do -- loosen up the hardcoded name "backend".
    backend: CanisterIdPerNetwork,
}

#[derive(Debug, Deserialize)]
struct CanisterIdPerNetwork {
    ic: Option<String>,
    local: Option<String>,
}

#[derive(StructOpt)]
struct Cli {
    #[structopt(long, short)]
    quiet: bool,

    #[structopt(long)]
    network: Option<String>,

    #[structopt(long)]
    canister_id: Option<String>,

    #[structopt(subcommand)]
    command: CliCommand,
}

#[derive(StructOpt, Debug)]
#[structopt(about = "Motoko MainMemory Snapshot Tool.")]
enum CliCommand {
    #[structopt(about = "Query canister for latest snapshot info.")]
    Info(InfoArgs),
    #[structopt(about = "Download latest snapshot image from canister, saving a local copy.")]
    Pull(PullArgs),
    #[structopt(about = "Evaluate a Motoko script over a snapshot image.")]
    Eval(EvalArgs),
    #[structopt(about = "Update canister with new snapshot region holding a new snapshot.")]
    Create(CreateArgs),
    #[structopt(
        about = "Update canister by overwriting last snapshot region with a new snapshot."
    )]
    Update(UpdateArgs),
    // todo --
    //
    //#[structopt(about = "Produce the hash of a local snapshot image file.")]
    //Hash(HashArgs),
}

#[derive(StructOpt, Debug, Default)]
struct InfoArgs {
    #[structopt(long, short)]
    quiet: bool,
}

#[derive(StructOpt, Debug, Default)]
struct CreateArgs {}

#[derive(StructOpt, Debug, Default)]
struct UpdateArgs {}

#[derive(StructOpt, Debug, Default)]
struct HashArgs {}

#[derive(StructOpt, Debug, Default)]
struct PullArgs {
    #[structopt(long, short, about = "target file for writing image.")]
    file: Option<String>,
}

#[derive(StructOpt, Debug, Default)]
struct EvalArgs {
    #[structopt(help = "Motoko program to evalluate over `image`, the main memory image value.")]
    program: String,

    #[structopt(help = "Source file for reading image.", long, short)]
    file: Option<String>,

    #[structopt(help = "Prints the parsed Motoko AST, verbosely.", long, short)]
    print_parse: bool,
}

#[derive(CandidType, Deserialize, Debug)]
struct SnapshotInfo {
    id: u32,
    pages: u64,
    time: i64,
}

impl SnapshotInfo {
    fn print_pretty(&self) {
        println!(" id = {}", self.id);

        println!(
            " time = {}",
            humantime::Timestamp::from(into_system_time(self.time))
        );

        let full_len = (1 << 16) * self.pages;

        println!(
            " size = {}",
            Byte::from_bytes(full_len as u128).get_appropriate_unit(true)
        );
    }
}

struct Context {
    quiet: bool,
    agent: Option<Agent>,
    canister_id: Option<Principal>,
}

impl Context {
    fn image_file_name(&self, time: i64) -> String {
        format!(
            "{}_{}.momm",
            self.canister_id.unwrap(),
            &Timestamp::from(into_system_time(time))
        )
    }
}

async fn go(cli: Cli) {
    // to do -- use network parameter, and default to one or the other.
    let url = "https://icp0.io";

    let need_agent = match &cli.command {
        Eval(e) => e.file.is_none(),
        _ => true,
    };

    // Invariant:
    //   need_agent implies canister_id_str will be Some(_)
    //
    let canister_id_str = match cli.canister_id {
        Some(s) => Some(s.clone()),
        None => {
            if !need_agent {
                None
            } else {
                if !cli.quiet {
                    println!("Need a canister ID, looking in canister_ids.json");
                }
                let Ok(file) = File::open("canister_ids.json") else {
		    println!("Missing: canister_id argument \nand canister_ids.json file (with `backend` canister).");
		    println!("Supply one and please try again.");
		    return
		};
                let reader = BufReader::new(file);
                let ids: CanisterIds = serde_json::from_reader(reader).expect("Invalid JSON");
                if !cli.quiet && ids.backend.ic.is_some() {
                    println!(
                        "Success: The value of 'ic' field within 'backend' is: {:?}",
                        &ids.backend.ic
                    );
                }
                if ids.backend.ic.is_none() {
                    println!("Missing: The value of 'ic' field within 'backend'.");
                }
                ids.backend.ic.clone()
            }
        }
    };

    let canister_id = if !need_agent {
        None
    } else {
        let Some(canister_id_str) = canister_id_str else {
	    println!("Error: Need a canister ID.");		    
	    return
	};
        Some(Principal::from_text(canister_id_str).expect("valid canister ID"))
    };

    let agent = if need_agent {
        if !cli.quiet {
            println!("Starting agent...");
        }
        let agent = Agent::builder()
            .with_transport(V2Transport::create(url).unwrap())
            .build()
            .unwrap();
        agent.fetch_root_key().await.unwrap();
        Some(agent)
    } else {
        None
    };

    let mut context = Context {
        quiet: cli.quiet,
        agent,
        canister_id,
    };

    use CliCommand::*;
    match cli.command {
        Info(i) => drop(info(&mut context, i).await),
        Pull(p) => pull(&mut context, p).await,
        Eval(e) => eval(&mut context, e).await,
        Create(c) => drop(create(&mut context, c).await),
        Update(u) => drop(update(&mut context, u).await),
    }
}

fn into_system_time(t: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH.add(std::time::Duration::from_nanos(t as u64))
}

async fn info(context: &mut Context, info_args: InfoArgs) -> Option<SnapshotInfo> {
    if !context.quiet {
        println!(
            "Reading last snapshot of {}...",
            context.canister_id.unwrap()
        );
    }

    let info_response = context
        .agent
        .as_mut()
        .expect("agent")
        .query(&context.canister_id.unwrap(), "getLastSnapshotInfo")
        .with_arg(encode_args(()).unwrap())
        .call()
        .await
        .unwrap();

    let info_result = Decode!(info_response.as_slice(), Option<SnapshotInfo>).unwrap();

    let Some(info_result) = info_result else {
	if !context.quiet {
	    println!("No snapshots for {}.", context.canister_id.unwrap());
	}
	return None
    };

    if !info_args.quiet {
        info_result.print_pretty();
    }

    return Some(info_result);
}

async fn create(context: &mut Context, _create_args: CreateArgs) -> SnapshotInfo {
    if !context.quiet {
        println!(
            "Creating new region for new snapshot of {}...",
            context.canister_id.unwrap()
        );
    }

    let info_response = context
        .agent
        .as_mut()
        .expect("agent")
        .update(&context.canister_id.unwrap(), "createSnapshot")
        .with_arg(encode_args(()).unwrap())
        .call_and_wait()
        .await
        .unwrap();

    let info_result = Decode!(info_response.as_slice(), SnapshotInfo).unwrap();

    info_result.print_pretty();

    if !context.quiet {
        println!("Done.");
    }

    info_result
}

async fn update(context: &mut Context, _update_args: UpdateArgs) -> Option<SnapshotInfo> {
    if !context.quiet {
        println!(
            "Updating last-used region for new snapshot of {}...",
            context.canister_id.unwrap()
        );
    }

    let info_response = context
        .agent
        .as_mut()
        .expect("agent")
        .update(&context.canister_id.unwrap(), "updateSnapshot")
        .with_arg(encode_args(()).unwrap())
        .call_and_wait()
        .await
        .unwrap();

    let info_result = Decode!(info_response.as_slice(), SnapshotInfo).unwrap();

    info_result.print_pretty();

    if !context.quiet {
        println!("Done.");
    }

    Some(info_result)
}

async fn pull(context: &mut Context, pull_args: PullArgs) {
    let Some(info_result) = info(context, InfoArgs{quiet: context.quiet}).await else {
	return
    };

    let file_path = match pull_args.file {
        None => context.image_file_name(info_result.time),
        Some(fp) => fp,
    };

    let mut file = File::create(&file_path).unwrap();

    if !context.quiet {
        println!("Opened {} for writing output.", file_path);
    }

    let full_len = (1 << 16) * info_result.pages;
    let mut pos = 0;
    let max_req = 2_000_000;

    let full_len_str = Byte::from_bytes(full_len as u128).get_appropriate_unit(true);

    loop {
        let rem = full_len - pos;
        let req = if rem > max_req { max_req } else { rem };

        if !context.quiet {
            println!(
                "Reading: offset {}, size {}",
                Byte::from_bytes(pos as u128).get_appropriate_unit(true),
                Byte::from_bytes(req as u128).get_appropriate_unit(true)
            );
        }

        let read_response = context
            .agent
            .as_ref()
            .expect("agent")
            .query(&context.canister_id.unwrap(), "readLastSnapshot")
            .with_arg(encode_args((pos as u64, Nat::from(req))).unwrap())
            .call()
            .await
            .unwrap();

        let result = Decode!(read_response.as_slice(), std::vec::Vec<u8>).unwrap();

        if !context.quiet {
            println!(" Writing ...");
        }
        file.write(&result).unwrap();
        if !context.quiet {
            println!(" Done.");
        }

        pos += req;
        if pos >= full_len {
            break;
        };
    }

    if !context.quiet {
        println!("Done: Read full main memory snapshot successfully.");
        println!("{}", full_len_str);
    }
}

/* ------------------------------------------------------------------------ */
//
// Evaluation
//
// - The `image` variable is bound to the image, as a memory-mapped file.
// - `image.size` gives the size in bytes,
//
// - to do : `image.vals` gives a bytes-based iterator.
// - to do : 'image.valsNat32` gives a worlds-based iterator.
/* ------------------------------------------------------------------------ */

#[derive(Debug)]
pub struct ImageValue {
    pub file_path: String,
    pub memmap: memmap::Mmap,
}

impl ImageValue {
    fn new(file_path: String) -> Self {
        let file = File::open(&file_path).unwrap();
        let memmap = unsafe { memmap::Mmap::map(&file).unwrap() };
        ImageValue { file_path, memmap }
    }
}
impl Eq for ImageValue {}
impl PartialEq for ImageValue {
    fn eq(&self, other: &Self) -> bool {
        self.file_path == other.file_path
    }
}

impl Clone for ImageValue {
    fn clone(&self) -> Self {
        ImageValue::new(self.file_path.clone())
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum ImageMethod {
    Size,
    Vals,
    ValsNat32,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ImageMethodValue {
    pub image: ImageValue,
    pub method: ImageMethod,
}

impl Hash for ImageValue {
    fn hash<H: Hasher>(&self, _state: &mut H) {
        panic!("do not hash Window method values, please");
    }
}

impl Dynamic for ImageMethodValue {
    fn call(&mut self, _store: &mut Store, _inst: &Option<Inst>, _args: Value_) -> MoRes {
        match self.method {
            ImageMethod::Size => Ok(Value::Nat(self.image.memmap.len().into()).share()),
            _ => todo!(),
        }
    }
}

impl Dynamic for ImageValue {
    fn get_field(&self, _store: &Store, name: &str) -> MoRes {
        if name == "size" {
            Ok(ImageMethodValue {
                image: self.clone(),
                method: ImageMethod::Size,
            }
            .into_value()
            .into())
        } else {
            todo!()
        }
    }
}

async fn eval(context: &mut Context, eval_args: EvalArgs) {
    let file_path = match eval_args.file {
        Some(fp) => fp,
        None => {
            let Some(info_result) = info(context, InfoArgs{quiet: context.quiet}).await else {
		return
	    };
            context.image_file_name(info_result.time)
        }
    };

    let image = ImageValue::new(file_path);

    let program = match motoko::check::parse(&eval_args.program) {
        Ok(p) => {
            if eval_args.print_parse {
                println!("parsed: {:?}", p);
            }
            p
        }
        Err(err) => {
            println!("syntax error: {:?}", err);
            return;
        }
    };

    let r = movm::update(|core| {
        core.eval_open_block(vec![("image", image.into_value().share())], program)
            .expect("program evaluation.")
    });

    // Print the debug output, if any.
    for line in movm::get().debug_print_out.iter() {
        println!("{:?}", line.text);
    }

    // Print the result value.
    println!("{:?}", r);
}

#[tokio::main]
async fn main() {
    let cli = Cli::from_args();
    go(cli).await;
}
