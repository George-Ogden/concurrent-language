grammar Grammar;

MULTILINE_COMMENT : '/*' .*? '*/' -> skip ;
SINGLE_LINE_COMMENT : '//' ~[\r\n]* -> skip ;

EQ : '=' ;
COMMA : ',' ;
SEMI : ';' ;
LPAREN : '(' ;
RPAREN : ')' ;
LCURLY : '{' ;
RCURLY : '}' ;
RIGHTARROW: '->' ;
LANGLE : '<' ;
RANGLE : '>' ;
PIPE : '|' ;
NEGATE: '-' ;
DOT: '.' ;

IF : 'if' ;
ELSE : 'else' ;
TYPEDEF : 'typedef' ;
TYPEALIAS : 'typealias' ;
MATCH : 'match' ;

OPERATOR: [&|=!/*+^$<>@:]+ ;
OPERATOR_ID: '__' [&|=!/*+^$<>@:]+ '__';
INFIX_ID: '__' [a-zA-Z_][a-zA-Z_0-9]* '__' ;
ID: [a-zA-Z_][a-zA-Z_0-9]* ;
UINT: '0' | [1-9][0-9]* ;
WS: [ \t\n\r\f]+ -> skip ;

program : imports definitions EOF ;

imports: ;

definitions : | (definition ';')+ definition? ;

definition
    : type_def
    | assignment
    | type_alias
//    | trait_def
//    | trait_impl
    ;

generic : '<' generic_list '>' ;
generic_list : | ID (',' ID)* ','? ;
generic_id : ID generic? ;

type_instance : return_type | fn_type | '(' type_instance ')';
return_type
    : generic_id
    | tuple_type
    ;

type_alias: TYPEALIAS generic_id type_instance;
type_def: TYPEDEF generic_id (
    union_def |
    type_instance |
//     record_def |
    empty_def
);

empty_def : ;

union_def : '{' type_item ('|' type_item )* '}' ;
type_item: ID type_instance ? ;
tuple_def : '(' type_list ')' ;

tuple_type : '(' type_list ')' ;
type_list : | (type_instance ',')+ type_instance?;

fn_type : fn_type_head fn_type_tail ;

fn_type_tail : RIGHTARROW type_instance ;

fn_type_head
    : return_type
    | '(' type_instance ')'
    ;

assignment : assignee '=' expr ;
assignment_list : | (assignment ';')*;

assignee
    : generic_id
    | OPERATOR_ID
//    | tuple_assignee
//    | record_assignee
    ;

infix_free_expr
    : integer
    | generic_id
    | if_expr
    | match_expr
//     | switch_expr
//     | record_expr
    | '(' expr ')'
    | tuple_expr
    | fn_def
    | fn_call
    ;

expr : infix_free_expr | infix_call;

integer: '-'? UINT;

fn_call : generic_id '(' (expr | expr_list) ')' ;

infix_operator
    : INFIX_ID
    | OPERATOR
    | DOT
    | NEGATE
    | PIPE
    | LANGLE
    | RANGLE
    ;

infix_call : infix_free_expr infix_operator expr;
tuple_expr: '(' expr_list ')';
expr_list : | (expr ',' )+ expr? ;

if_expr : IF '(' expr ')' block ELSE block ;
match_expr : MATCH '(' expr ')' '{' match_block (';' match_block)* ';' '}' ;
match_block : ID assignee ? ('|' ID assignee ?)* ':' block ;

fn_def : '(' typed_assignee_list ')' RIGHTARROW type_instance block;
typed_assignee_list : | typed_assignee (',' typed_assignee)* ',' ?;
typed_assignee : assignee ':' type_instance ;

block : '{' assignment_list expr '}' ;
