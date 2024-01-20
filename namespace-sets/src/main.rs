use diesel::pg::PgConnection;
use diesel::prelude::*;
use std::env;

use petgraph::dot::{Config, Dot};
use petgraph::graph::DiGraph;
use std::fs::File;
use std::io::Write;
use std::process::Command;

mod schema;

use crate::schema::nested_category;

// Assuming `nested_category` is a Diesel model
#[derive(Queryable, Debug, Clone)]
pub struct NestedCategory {
    pub id: i32,
    pub lft: i32,
    pub rgt: i32,
    pub name: String,
}

fn find_parent_from_categories(
    categories: &[NestedCategory],
    child: &NestedCategory,
) -> Option<NestedCategory> {
    categories
        .iter()
        .filter(|&cat| cat.lft < child.lft && cat.rgt > child.rgt)
        .max_by_key(|cat| cat.lft)
        .cloned()
}

#[allow(dead_code)]
fn find_parent_from_db(
    conn: &mut PgConnection,
    child: &NestedCategory,
) -> QueryResult<Option<NestedCategory>> {
    println!("Finding parent for {:?}", child);
    nested_category::table
        .filter(nested_category::lft.lt(child.lft))
        .filter(nested_category::rgt.gt(child.rgt))
        .order(nested_category::lft.desc())
        .first(conn)
        .optional()
}

fn build_graph(connection: &mut PgConnection) -> DiGraph<String, ()> {
    let categories = nested_category::table
        .load::<NestedCategory>(connection)
        .expect("Unable to load categories");

    let mut graph = DiGraph::new();
    let mut node_indices = std::collections::HashMap::new();

    for cat in categories.clone() {
        let node_format = format!("{} [ {},{} ]", cat.name, cat.lft, cat.rgt);
        let index = graph.add_node(node_format);
        node_indices.insert(cat.id, index);
    }

    for cat in categories.clone() {
        if let Some(parent) = find_parent_from_categories(&categories, &cat) {
            if let Some(parent_index) = node_indices.get(&parent.id) {
                let child_index = node_indices.get(&cat.id).unwrap();
                graph.add_edge(*parent_index, *child_index, ());
            }
        }
    }

    graph
}

fn export_to_png(graph: DiGraph<String, ()>, filename: &str) {
    let dot = format!("{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));

    let mut dot_file = File::create(format!("{}.dot", filename)).unwrap();
    dot_file.write_all(dot.as_bytes()).unwrap();

    Command::new("dot")
        .args(&[
            "-Tpng",
            &format!("{}.dot", filename),
            "-o",
            &format!("{}.png", filename),
        ])
        .output()
        .expect("failed to execute process");
}

fn find_ancestors(conn: &mut PgConnection, node_name: &str) -> QueryResult<Vec<NestedCategory>> {
    let node = nested_category::table
        .filter(nested_category::name.eq(node_name))
        .first::<NestedCategory>(conn)?;

    nested_category::table
        .filter(nested_category::lft.lt(node.lft))
        .filter(nested_category::rgt.gt(node.rgt))
        .order(nested_category::lft)
        .load::<NestedCategory>(conn)
}

pub fn find_descendants(
    conn: &mut PgConnection,
    node_name: &str,
) -> QueryResult<Vec<NestedCategory>> {
    let node = nested_category::table
        .filter(nested_category::name.eq(node_name))
        .first::<NestedCategory>(conn)?;

    nested_category::table
        .filter(nested_category::lft.gt(node.lft))
        .filter(nested_category::rgt.lt(node.rgt))
        .order(nested_category::lft)
        .load::<NestedCategory>(conn)
}

fn establish_connection() -> PgConnection {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

pub fn show_category(category: &str) {
    println!("Category: {}", category);
    let mut connection = establish_connection();
    let category = nested_category::table
        .filter(nested_category::name.eq(category))
        .first::<NestedCategory>(&mut connection)
        .expect("Error loading category");

    println!(" {:?}", category);
    show_ancestors(category.name.as_str());
    show_descendants(category.name.as_str());
}

pub fn show_ancestors(category: &str) {
    println!("Ancestors of category: {}", category);
    for ancestor in find_ancestors(&mut establish_connection(), category).unwrap() {
        println!(" {:?}", ancestor);
    }
}

pub fn show_descendants(category: &str) {
    println!("Descendants of category: {}", category);
    for descendant in find_descendants(&mut establish_connection(), category).unwrap() {
        println!(" {:?}", descendant);
    }
}

pub fn list_categories() {
    let mut connection = establish_connection();
    let categories = nested_category::table
        .load::<NestedCategory>(&mut connection)
        .expect("Unable to load categories");

    println!("Listing categories:");
    println!("ID  Name                 LFT RGT");
    for category in categories {
        println!(
            "{:03} {:20} {:03} {:03}",
            category.id, category.name, category.lft, category.rgt
        );
    }
}

pub fn create_root_category_if_not_exists(
    name: &str,
) -> Result<NestedCategory, diesel::result::Error> {
    let mut connection = establish_connection();
    let root_category = nested_category::table
        .filter(nested_category::name.eq(name))
        .first::<NestedCategory>(&mut connection);

    match root_category {
        Ok(category) => Ok(category),
        Err(_) => create_root_category(name),
    }
}

pub fn create_root_category(name: &str) -> Result<NestedCategory, diesel::result::Error> {
    let mut connection = establish_connection();
    let root_category = diesel::insert_into(nested_category::table)
        .values((
            nested_category::name.eq(name),
            nested_category::lft.eq(1),
            nested_category::rgt.eq(2),
        ))
        .get_result::<NestedCategory>(&mut connection)?;

    Ok(root_category)
}

pub fn add_category(parent: &str, new: &str) -> Result<NestedCategory, diesel::result::Error> {
    let mut connection = establish_connection();

    connection.transaction::<NestedCategory, diesel::result::Error, _>(|connection| {
        let parent_node: NestedCategory = nested_category::table
            .filter(nested_category::name.eq(parent))
            .first(connection)
            .expect("Error loading parent node");

        let my_right = parent_node.rgt;

        diesel::update(nested_category::table.filter(nested_category::rgt.ge(my_right)))
            .set(nested_category::rgt.eq(nested_category::rgt + 2))
            .execute(connection)?;

        diesel::update(nested_category::table.filter(nested_category::lft.gt(my_right)))
            .set(nested_category::lft.eq(nested_category::lft + 2))
            .execute(connection)?;

        let new_category = diesel::insert_into(nested_category::table)
            .values((
                nested_category::name.eq(new),
                nested_category::lft.eq(my_right),
                nested_category::rgt.eq(my_right + 1),
            ))
            .get_result::<NestedCategory>(connection)?;

        Ok(new_category)
    })
}

fn main() {
    let args: Vec<String> = env::args().collect();
    create_root_category_if_not_exists("root").expect("Error creating root category");

    if args.len() == 2 {
        show_category(&args[1]);
        return;
    }

    if args.len() != 3 {
        list_categories();

        let mut connection = establish_connection();
        let graph = build_graph(&mut connection);
        export_to_png(graph, "category_tree");

        return;
    }

    let parent_category = &args[1];
    let new_category_name = &args[2];

    let new_category =
        add_category(parent_category, new_category_name).expect("Error adding category");
    println!("Added category: {:?}", new_category);
    list_categories();
}
