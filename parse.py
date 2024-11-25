import re
import sys
from parser.GrammarLexer import GrammarLexer
from parser.GrammarParser import GrammarParser
from parser.GrammarVisitor import GrammarVisitor
from typing import Optional

from antlr4 import *
from antlr4.tree.Trees import Trees

from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    Block,
    Boolean,
    FunctionCall,
    FunctionType,
    GenericVariable,
    Integer,
    TupleExpression,
    TupleType,
)


def main(argv):
    input_stream = InputStream(argv[1])
    target = sys.argv[2] if len(sys.argv) >= 3 else "program"
    lexer = GrammarLexer(input_stream)
    stream = CommonTokenStream(lexer)
    parser = GrammarParser(stream)
    assert target in parser.ruleNames
    if target == "id":
        target = "id_"
    tree = getattr(parser, target).__call__()
    print(Trees.toStringTree(tree, None, parser))


class Visitor(GrammarVisitor):
    def visitList(self, ctx: ParserRuleContext):
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitId(self, ctx: GrammarParser.IdContext):
        return ctx.getText()

    def visitAtomic_type(self, ctx: GrammarParser.Atomic_typeContext):
        type_name = ctx.getText().upper()
        type = AtomicTypeEnum[type_name]
        return AtomicType(type)

    def visitType_instance(self, ctx: GrammarParser.Type_instanceContext):
        if ctx.type_instance() is not None:
            return self.visit(ctx.type_instance())
        return super().visitType_instance(ctx)

    def visitGeneric_list(self, ctx: GrammarParser.Generic_listContext):
        return self.visitList(ctx)

    def visitGeneric_instance(self, ctx: GrammarParser.Generic_instanceContext):
        id = self.visitId(ctx.id_())
        generic_list = [] if ctx.generic_list() is None else self.visit(ctx.generic_list())
        return GenericVariable(id, generic_list)

    def visitType_list(self, ctx: GrammarParser.Type_listContext):
        return self.visitList(ctx)

    def visitTuple_type(self, ctx: GrammarParser.Tuple_typeContext):
        return TupleType(self.visit(ctx.type_list()))

    def visitFn_type_head(self, ctx: GrammarParser.Fn_type_headContext):
        if ctx.return_type() is not None:
            return self.visit(ctx.return_type())
        else:
            return self.visit(ctx.type_instance())

    def visitFn_type(self, ctx: GrammarParser.Fn_typeContext):
        argument_types = self.visit(ctx.fn_type_head())
        return_type = self.visit(ctx.fn_type_tail())
        return FunctionType(argument_types, return_type)

    def visitInteger(self, ctx: GrammarParser.IntegerContext):
        value = int(ctx.getText())
        return Integer(value)

    def visitBoolean(self, ctx: GrammarParser.BooleanContext):
        if ctx.getText().lower() == "true":
            return Boolean(True)
        else:
            return Boolean(False)

    def visitExpr_list(self, ctx: GrammarParser.Expr_listContext):
        return self.visitList(ctx)

    def visitTuple_expr(self, ctx: GrammarParser.Tuple_exprContext):
        expressions = self.visit(ctx.expr_list())
        return TupleExpression(expressions)

    def visitInfix_operator(self, ctx: GrammarParser.Infix_operatorContext):
        operator = ctx.getText()
        match = re.match(r"^__(\S+)__$", ctx.getText())
        if match:
            operator = match.group(1)
        return operator

    def visitInfix_free_expr(self, ctx: GrammarParser.Infix_free_exprContext):
        if ctx.expr() is not None:
            return self.visit(ctx.expr())
        [child] = ctx.getChildren()
        if ctx.generic_instance() is not None:
            return self.visitGeneric_instance(child)
        return super().visitInfix_free_expr(ctx)

    def visitInfix_call(self, ctx: GrammarParser.Infix_callContext):
        left = self.visit(ctx.infix_free_expr())
        operator = self.visit(ctx.infix_operator())
        right = self.visit(ctx.expr())
        return FunctionCall(operator, [], [left, right])

    def visitId_list(self, ctx: GrammarParser.Id_listContext):
        return self.visitList(ctx)

    def visitGeneric_target(self, ctx: GrammarParser.Generic_targetContext):
        id = self.visit(ctx.id_())
        generics = [] if ctx.id_list() is None else self.visit(ctx.id_list())
        return Assignee(id, generics)

    def visitAssignee(self, ctx: GrammarParser.AssigneeContext):
        if ctx.operator_id() is not None:
            id = re.match(r"^__(\S+)__$", ctx.getText()).group(1)
            return Assignee(id, [])
        elif ctx.getText() == "__":
            return Assignee("__", [])
        return super().visit(ctx.generic_target())

    def visitAssignment(self, ctx: GrammarParser.AssignmentContext):
        assignee = self.visit(ctx.assignee())
        expression = self.visit(ctx.expr())
        return Assignment(assignee, expression)

    def visitAssignment_list(self, ctx: GrammarParser.Assignment_listContext):
        return self.visitList(ctx)

    def visitBlock(self, ctx: GrammarParser.BlockContext):
        assignments = self.visit(ctx.assignment_list())
        expression = self.visit(ctx.expr())
        return Block(assignments, expression)


class Parser:
    @staticmethod
    def parse(code: str, target: str) -> Optional[ASTNode]:
        input_stream = InputStream(code)
        lexer = GrammarLexer(input_stream)
        stream = CommonTokenStream(lexer)
        parser = GrammarParser(stream)
        if target in parser.ruleNames:
            tree = getattr(parser, target).__call__()
            if parser.getNumberOfSyntaxErrors() > 0 or stream.LA(1) != Token.EOF:
                return None
            visitor = Visitor()
            return visitor.visit(tree)


if __name__ == "__main__":
    main(sys.argv)
