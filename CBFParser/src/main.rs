use std::env;
use std::fs::File;
use common::raf::Raf;
use ctf::cff_header;
use std::io::Read;

mod caesar;
mod ctf;

fn help(err: String) -> ! {
    println!("Error: {}", err);
    println!("Usage:");
    println!("cbf_parser <INPUT.CBF>");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        2 => read_file(&args[1]),
        _ => help(format!("Invalid number of args: {}", args.len() - 1)),
    }
}

fn read_file(path: &String) {
    if path.ends_with(".cff") {
        eprintln!("Cannot be used with CFF. Only CBF!");
        return;
    }
    let mut f = File::open(path).expect("Cannot open input file");
    let mut buffer = vec![0; f.metadata().unwrap().len() as usize];
    f.read_exact(&mut buffer).expect("Error reading file");
    println!("Have {} bytes", buffer.len());
    let mut br = Raf::from_bytes(&buffer, common::raf::RafByteOrder::LE);

    let header = br.read_bytes(ctf::STUB_HEADER_SIZE).expect("Could not read header bytes");
    ctf::StubHeader::read_header(&header);

    let header_size = br.read_u32().expect("Oops");
    br.read_bytes(header_size as usize);

    let res = cff_header::CFFHeader::new(&mut br);
    match res {
        Ok(header) => println!("{:#?}", header),
        Err(e) => eprintln!("{:?}", e)
    }
}
