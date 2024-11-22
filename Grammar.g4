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
PIPE : '|' ;
RIGHTARROW: '->' ;
NEGATE: '-' ;
DOT: '.' ;

IF : 'if' ;
ELSE : 'else' ;
TYPE : 'type' ;
MATCH : 'match' ;

OPERATOR: [&|=!/*+^$<>@:]+ ;
OPERATOR_ID: '__' [&|=!/*+^$<>@:]+ '__';
INFIX_ID: '__' [a-zA-Z_][a-zA-Z_0-9]* '__' ;
ID: [a-zA-Z_][a-zA-Z_0-9]* ;
UINT: '0' | [1-9][0-9]* ;
WS: [ \t\n\r\f]+ -> skip ;

program : imports defs EOF ;

imports: ;

defs : | def (';' def)*  ';'? ;

def
    : type_def
    | assignment
//    | trait_def
//    | trait_impl
    ;

type : return_type | fn_type | '(' type ')';
return_type
    : ID
    | tuple_type
    ;

type_def: TYPE ID (
    union_def |
    type |
//     record_def |
    empty_def
);

empty_def : ;

union_def : '{' type_item ('|' type_item )* '}' ;
type_item: ID type ? ;
tuple_def : '(' type_list ')' ;

tuple_type : '(' type_list ')' ;
type_list : | (type ',')+ type?;

fn_type : fn_type_head fn_type_tail ;

fn_type_tail : RIGHTARROW type ;

fn_type_head
    : return_type
    | '(' type ')'
    ;

assignment : assignee '=' expr ;
assignment_list : | (assignment ';')+ assignment? ;

assignee
    : ID
    | OPERATOR_ID
//    | tuple_assignee
//    | record_assignee
    ;

infix_access_free_expr
    : value
    | if_expr
    | match_expr
//     | switch_expr
//     | record
    | '(' expr ')'
    | tuple
    | fn_def
    | fn_call
    ;

expr : infix_access_free_expr | infix_call | access;

value
    : int
//    | STRING
    | ID
    ;

int: '-'? UINT;

fn_call : ID '(' (expr | expr_list) ')' ;

infix_operator
    : INFIX_ID
    | OPERATOR
    | DOT
    | NEGATE
    | PIPE
    ;

infix_call : infix_access_free_expr infix_operator expr;
tuple: '(' expr_list ')';
expr_list : | (expr ',' )+ expr? ;

if_expr : IF '(' expr ')' block ELSE block ;
match_expr : MATCH '(' expr ')' '{' match_block (';' match_block)* ';' '}' ;
match_block : ID assignee ? ('|' ID assignee ?)* ':' block ;

fn_def : '(' typed_assignee_list ')' RIGHTARROW type block;
typed_assignee_list : | (typed_assignee ',')+ typed_assignee ?;
typed_assignee : assignee ':' type ;

access: access_head access_tail;

access_tail : DOT (ID | UINT | access_tail);
access_head : infix_access_free_expr | infix_call ;
block : '{' assignment_list expr '}' ;
