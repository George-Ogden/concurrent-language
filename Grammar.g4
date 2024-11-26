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
UNDER: '_';

IF : 'if' ;
ELSE : 'else' ;
TYPEDEF : 'typedef' ;
TYPEALIAS : 'typealias' ;
MATCH : 'match' ;
INT: 'int';
TRUE: 'true';
FALSE: 'false';
BOOL: 'bool';

INFIX_ID: '_''_'[a-zA-Z_][a-zA-Z0-9_]*'_''_' ;
ID: [a-zA-Z_][a-zA-Z0-9_]* ;
UINT: '0' | [1-9][0-9]* ;
WS: [ \t\n\r\f]+;

program : imports definitions EOF ;

imports: ;

definitions : | (definition WS* ';' WS* )+ definition? ;

definition
    : type_def
    | assignment
    | type_alias
//    | trait_def
//    | trait_impl
    ;

id: ID | '_' | INFIX_ID;
operator_symbol_without_eq_dot: ('&' | '|' | '!' | '+' | '-' | '^' | '$' | '<' | '>' | '@' | ':' | '*' | '%' | '/');
operator_symbol: operator_symbol_without_eq_dot | '=' | '.';
operator: (operator_symbol)+ operator_symbol | operator_symbol_without_eq_dot;
operator_id: '__' operator '__';

id_list : | id  WS* (',' WS* id WS* )* ','? ;
generic_assignee : id ('<' WS* id_list WS* '>')? ;

generic_list : | type_instance WS* (',' WS* type_instance WS*)* ','? WS* ;
generic_instance : id ('<' generic_list '>')? ;

atomic_type
    : BOOL
    | INT
    ;

type_instance : return_type | fn_type | '(' type_instance ')';
generic_type_instance: generic_instance;
return_type
    : generic_type_instance
    | atomic_type
    | tuple_type
    ;

type_alias: TYPEALIAS WS+ generic_typevar WS* type_instance;
generic_typevar: generic_assignee;
type_def: TYPEDEF WS+ generic_typevar WS * (
    union_def |
    type_instance |
//     record_def |
    empty_def
);

empty_def : WS*;

union_def : '{' WS* type_item WS* ('|' WS* type_item WS* )+ '}' ;
type_item: id WS* type_instance ? ;
tuple_def : '(' WS*  type_list WS* ')' ;

tuple_type : '(' WS*  type_list WS* ')' ;
type_list : | (type_instance WS * ',' WS*)+ type_instance?;

fn_type : fn_type_head WS* fn_type_tail ;

fn_type_tail : RIGHTARROW WS* type_instance ;

fn_type_head
    : return_type
    | '(' WS* type_instance WS* ')'
    ;

assignment : assignee WS*  '=' WS* expr ;
assignment_list : | (assignment WS* ';' WS*)*;

assignee
    : generic_assignee
    | operator_id
    | '__'
//    | tuple_assignee
//    | record_assignee
    ;

fn_call_access_free_expr
    : integer
    | boolean
    | generic_instance
    | if_expr
    | match_expr
//     | switch_expr
//     | record_expr
    | '(' WS* expr WS* ')'
    | tuple_expr
    | fn_def
    | prefix_call
    ;

fn_call_free_expr: fn_call_access_free_expr | access;
access: access_head access_tail;
access_head: fn_call_access_free_expr;
access_tail: DOT UINT access_tail?;

infix_free_expr: fn_call_free_expr | fn_call;
expr: infix_free_expr | infix_call;

integer: '-'? UINT;
boolean: TRUE | FALSE;

fn_call: fn_call_head fn_call_tail;
fn_call_head: fn_call_free_expr;
fn_call_tail: '(' WS* (expr | expr_list) WS* ')' fn_call_tail?;

infix_operator
    : INFIX_ID
    | operator
    | NEGATE
    | PIPE
    | LANGLE
    | RANGLE
    | WS+ DOT WS+
    ;

prefix_call: infix_operator WS* expr;
infix_call: infix_free_expr WS* infix_operator WS* expr;
tuple_expr: '(' WS* expr_list WS* ')';
expr_list : | (expr WS* ',' WS* )+ expr? ;

if_expr : IF WS* '(' WS* expr WS* ')' WS* block WS* ELSE WS* block ;
match_expr : MATCH WS* '(' WS* expr WS* ')' WS* '{' WS* match_block_list WS* '}' ;
match_block_list : (WS* match_block WS* ';')* WS* match_block? ;
match_block : match_list WS* ':' WS* block ;
match_list : match_item (WS* '|' WS* match_item)*;
match_item: id WS* assignee ?;

fn_def : '(' WS* typed_assignee_list WS* ')' WS* RIGHTARROW WS* type_instance WS* block;
typed_assignee_list : | typed_assignee (WS* ',' WS* typed_assignee)* ',' ?;
typed_assignee : assignee WS* ':' WS* type_instance ;

block : '{' WS* assignment_list WS* expr WS* '}' ;
