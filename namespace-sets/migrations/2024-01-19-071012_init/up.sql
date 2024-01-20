-- Your SQL goes here
create table nested_category (
    id serial primary key,
    lft int not null,
    rgt int not null,
    name varchar(255) not null
);