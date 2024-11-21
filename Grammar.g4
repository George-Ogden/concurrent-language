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

TYPE : 'type' ;

ID: [a-zA-Z_][a-zA-Z_0-9]* ;
WS: [ \t\n\r\f]+ -> skip ;


program
    : type_def (';' type_def)*  ';'? EOF
    | EOF
    ;

type_def: TYPE ID type_expr;


type_expr : return_type | fn_type;

return_type
    : ID
    | tuple_type
    | union_type
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
