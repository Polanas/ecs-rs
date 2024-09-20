use regex::Regex;

fn main() {
    let re = Regex::new(r"\(\$(\w+) (\w+)\)").unwrap();
    let Some(caps) = re.captures("($Likes Apples)") else {
        println!("no match!");
        return;
    };
    dbg!(caps);
}
