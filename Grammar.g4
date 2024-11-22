grammar Grammar;

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

TYPE : 'type' ;

OPERATOR: [&|=!/*+^$<>]+ ;
INFIX_ID: '_' '_' [a-zA-Z_][a-zA-Z_0-9]* '_' '_' ;
ID: [a-zA-Z_][a-zA-Z_0-9]* ;
UINT: [1-9][0-9]* ;
WS: [ \t\n\r\f]+ -> skip ;

program : imports defs EOF ;

imports: ;

defs
    : def (';' def)*  ';'?
    |
    ;

def
    : type_def
    | assignment
//    | trait_def
//    | trait_impl
    ;

type_def: TYPE ID type_expr;


type_expr : return_type | fn_type;

return_type
    : ID
    | tuple_type
    | union_type
//     | record_type
    ;

tuple_type
    : '(' (type_expr ',')+ type_expr? ')'
    | '(' ')'
    ;

union_type
    : '{' ID type_expr ('|' ID type_expr)* '}'
    ;

fn_type : fn_type_head fn_type_tail ;

fn_type_head
    : return_type RIGHTARROW fn_type_head
    |   /* epsilon */
    ;

fn_type_tail
    : return_type
    | '(' fn_type ')'
    ;

assignment: assignee '=' expr ;

assignee
    : ID
//    | tuple_assignee
//    | record_assignee
    ;

infix_free_expr
    : value
//    | if_expr
//    | match_expr
//    | tuple
//    | record
//    | '(' expr ')'
//    | fn_def
//    | access
   | fn_call
    ;

expr : infix_free_expr | infix_call ;

value
    : int
//    | STRING
    | ID
    ;

int: '-'? UINT;

fn_call : ID '(' expr_list ')' ;
infix_call : infix_free_expr (OPERATOR | INFIX_ID) expr;
expr_list : | expr (',' expr)* ','? ;
