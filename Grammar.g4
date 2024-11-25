grammar Grammar;

@parser::members {
def check_for_ws(self):
    token_stream = self.getInputStream()
    la1 = token_stream.LT(1)
    lb1 = token_stream.LT(-1)

    for i in range(la1.tokenIndex - 1, -1, -1):
        token = token_stream.get(i)

        if token.channel != Token.HIDDEN_CHANNEL:
            return True

        if token.type == self.WS:
            return False

    return True

}

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
WS: [ \t\n\r\f]+ -> channel(HIDDEN);

not_ws: {self.check_for_ws()}? ;

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

id: ID | '_' | INFIX_ID;
operator_symbol_without_eq: ('&' | '|' | '!' | '+' | '-' | '^' | '$' | '<' | '>' | '@' | ':' | '*' | '%' | '/');
operator_symbol: operator_symbol_without_eq | '=' ;
operator: (operator_symbol not_ws)+ operator_symbol | operator_symbol_without_eq;
operator_id: '__' not_ws operator not_ws '__';

id_list : | id (',' id)* ','? ;
generic_target : id ('<' id_list '>')? ;

generic_list : | type_instance (',' type_instance)* ','? ;
generic_instance : id ('<' generic_list '>')? ;

atomic_type
    : BOOL
    | INT
    ;

type_instance : return_type | fn_type | '(' type_instance ')';
return_type
    : generic_instance
    | atomic_type
    | tuple_type
    ;

type_alias: TYPEALIAS generic_instance type_instance;
type_def: TYPEDEF generic_target (
    union_def |
    type_instance |
//     record_def |
    empty_def
);

empty_def : ;

union_def : '{' type_item ('|' type_item )* '}' ;
type_item: id type_instance ? ;
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
    : generic_target
    | operator_id
    | '__'
//    | tuple_assignee
//    | record_assignee
    ;

fn_call_free_expr
    : integer
    | boolean
    | generic_instance
    | if_expr
    | match_expr
//     | switch_expr
//     | record_expr
    | '(' expr ')'
    | tuple_expr
    | fn_def
    ;

infix_free_expr: fn_call_free_expr | fn_call;
expr: infix_free_expr | infix_call;

integer: '-'? UINT;
boolean: TRUE | FALSE;

fn_call: fn_call_head fn_call_tail;
fn_call_head: fn_call_free_expr;
fn_call_tail: '(' (expr | expr_list) ')' fn_call_tail?;

infix_operator
    : INFIX_ID
    | operator
    | DOT
    | NEGATE
    | PIPE
    | LANGLE
    | RANGLE
    ;

infix_call: infix_free_expr infix_operator expr;
tuple_expr: '(' expr_list ')';
expr_list : | (expr ',' )+ expr? ;

if_expr : IF '(' expr ')' block ELSE block ;
match_expr : MATCH '(' expr ')' '{' match_block_list '}' ;
match_block_list : (match_block ';')* match_block? ;
match_block : match_list ':' block ;
match_list : match_item ('|' match_item)*;
match_item: id assignee ?;

fn_def : '(' typed_assignee_list ')' RIGHTARROW type_instance block;
typed_assignee_list : | typed_assignee (',' typed_assignee)* ',' ?;
typed_assignee : assignee ':' type_instance ;

block : '{' assignment_list expr '}' ;
