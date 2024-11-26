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
    ElementAccess,
    EmptyTypeDefinition,
    FunctionCall,
    FunctionDef,
    FunctionType,
    GenericType,
    GenericTypeVariable,
    GenericVariable,
    IfExpression,
    Integer,
    MatchBlock,
    MatchExpression,
    MatchItem,
    OpaqueTypeDefinition,
    TupleExpression,
    TupleType,
    TypedAssignee,
    TypeItem,
    UnionTypeDefinition,
)
from operators import Associativity, OperatorManager


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


class VisitorError(Exception): ...


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

    def visitGeneric_type_instance(self, ctx: GrammarParser.Generic_type_instanceContext):
        generic_instance: GenericVariable = self.visit(ctx.generic_instance())
        return GenericType(generic_instance.name, generic_instance.type_variables)

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
        if ctx.fn_call_free_expr() is None:
            return self.visit(ctx.fn_call())
        else:
            return self.visit(ctx.fn_call_free_expr())

    def visitInfix_call(self, ctx: GrammarParser.Infix_callContext, carry=None):
        if carry is None:
            carry = ("id", lambda x: x)

        left = self.visit(ctx.infix_free_expr())
        operator = self.visit(ctx.infix_operator())

        parent_operator, tree = carry
        if (
            operator == parent_operator
            and OperatorManager.get_associativity(operator) == Associativity.NONE
        ):
            raise VisitorError(f"{operator} is non-associative")
        if operator == ".":

            def function(x):
                if not isinstance(x, Integer) or x.value < 0:
                    raise VisitorError(f"Invalid attribute {x}.")
                return ElementAccess(tree(left), x.value)

            carry = (operator, function)
        elif OperatorManager.get_precedence(parent_operator) < OperatorManager.get_precedence(
            operator
        ) or (
            operator == parent_operator
            and OperatorManager.get_associativity(operator) == Associativity.RIGHT
        ):
            carry = (
                operator,
                lambda x: FunctionCall(GenericVariable(operator, []), [tree(left), x]),
            )
        else:
            carry = (
                parent_operator,
                lambda x: tree(FunctionCall(GenericVariable(operator, []), [left, x])),
            )
        if ctx.expr().infix_call() is None:
            _, function = carry
            right = self.visit(ctx.expr())
            return function(right)
        else:
            return self.visitInfix_call(ctx.expr().infix_call(), carry=carry)

    def visitFn_call(self, ctx: GrammarParser.Fn_callContext):
        function = self.visit(ctx.fn_call_head())
        return self.visitFn_call_tail(ctx.fn_call_tail(), function=function)

    def visitFn_call_free_expr(self, ctx: GrammarParser.Fn_call_free_exprContext):
        if ctx.expr() is not None:
            return self.visit(ctx.expr())
        return super().visitFn_call_free_expr(ctx)

    def visitFn_call_tail(self, ctx: GrammarParser.Fn_call_tailContext, function=None):
        if ctx.expr() is None:
            args = self.visit(ctx.expr_list())
        else:
            args = [self.visit(ctx.expr())]
        function = FunctionCall(function, args)
        if ctx.fn_call_tail() is not None:
            return self.visitFn_call_tail(ctx.fn_call_tail(), function)
        return function

    def visitId_list(self, ctx: GrammarParser.Id_listContext):
        return self.visitList(ctx)

    def visitGeneric_assignee(self, ctx: GrammarParser.Generic_assigneeContext):
        id = self.visit(ctx.id_())
        generics = [] if ctx.id_list() is None else self.visit(ctx.id_list())
        return Assignee(id, generics)

    def visitAssignee(self, ctx: GrammarParser.AssigneeContext):
        if ctx.operator_id() is not None:
            id = re.match(r"^__(\S+)__$", ctx.getText()).group(1)
            return Assignee(id, [])
        elif ctx.getText() == "__":
            return Assignee("__", [])
        return super().visit(ctx.generic_assignee())

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

    def visitIf_expr(self, ctx: GrammarParser.If_exprContext):
        condition = self.visit(ctx.expr())
        true_ctx, false_ctx = ctx.block()
        true_block = self.visit(true_ctx)
        false_block = self.visit(false_ctx)
        return IfExpression(condition, true_block, false_block)

    def visitMatch_item(self, ctx: GrammarParser.Match_itemContext):
        name = self.visit(ctx.id_())
        assignee = None if ctx.assignee() is None else self.visit(ctx.assignee())
        return MatchItem(name, assignee)

    def visitMatch_list(self, ctx: GrammarParser.Match_listContext):
        return self.visitList(ctx)

    def visitMatch_block(self, ctx: GrammarParser.Match_blockContext):
        matches = self.visit(ctx.match_list())
        block = self.visit(ctx.block())
        return MatchBlock(matches, block)

    def visitMatch_block_list(self, ctx: GrammarParser.Match_block_listContext):
        return self.visitList(ctx)

    def visitMatch_expr(self, ctx: GrammarParser.Match_exprContext):
        subject = self.visit(ctx.expr())
        blocks = self.visit(ctx.match_block_list())
        return MatchExpression(subject, blocks)

    def visitTyped_assignee(self, ctx: GrammarParser.Typed_assigneeContext):
        assignee = self.visit(ctx.assignee())
        type_instance = self.visit(ctx.type_instance())
        return TypedAssignee(assignee, type_instance)

    def visitTyped_assignee_list(self, ctx: GrammarParser.Typed_assignee_listContext):
        return self.visitList(ctx)

    def visitFn_def(self, ctx: GrammarParser.Fn_defContext):
        assignees = self.visit(ctx.typed_assignee_list())
        return_type = self.visit(ctx.type_instance())
        body = self.visit(ctx.block())
        return FunctionDef(assignees, return_type, body)

    def visitGeneric_typevar(self, ctx: GrammarParser.Generic_typevarContext):
        assignee: Assignee = self.visit(ctx.generic_assignee())
        return GenericTypeVariable(assignee.id, assignee.generic_variables)

    def visitType_item(self, ctx: GrammarParser.Type_itemContext):
        id = self.visit(ctx.id_())
        type_instance = None if ctx.type_instance() is None else self.visit(ctx.type_instance())
        return TypeItem(id, type_instance)

    def visitUnion_def(self, ctx: GrammarParser.Union_defContext):
        return self.visitList(ctx)

    def visitType_def(self, ctx: GrammarParser.Type_defContext):
        type_variable: GenericTypeVariable = self.visit(ctx.generic_typevar())
        if ctx.type_instance() is not None:
            type_instance = self.visit(ctx.type_instance())
            return OpaqueTypeDefinition(type_variable, type_instance)
        elif ctx.empty_def() is not None:
            if type_variable.generic_variables != []:
                raise VisitorError(
                    f"Invalid empty type with generics {type_variable.generic_variables}"
                )
            return EmptyTypeDefinition(type_variable.id)
        else:
            type_items = self.visit(ctx.union_def())
            return UnionTypeDefinition(type_variable, type_items)


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
            try:
                return visitor.visit(tree)
            except VisitorError:
                return None


if __name__ == "__main__":
    main(sys.argv)
