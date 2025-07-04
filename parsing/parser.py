import re
from typing import Optional

from antlr4 import CommonTokenStream, InputStream, ParserRuleContext, Token
from ast_nodes import (
    Assignee,
    Assignment,
    ASTNode,
    AtomicType,
    AtomicTypeEnum,
    Block,
    Boolean,
    ConstructorCall,
    Definition,
    ElementAccess,
    EmptyTypeDefinition,
    Expression,
    FunctionCall,
    FunctionDefinition,
    FunctionType,
    GenericConstructor,
    GenericType,
    GenericTypeVariable,
    GenericVariable,
    IfExpression,
    Integer,
    MatchBlock,
    MatchExpression,
    MatchItem,
    OpaqueTypeDefinition,
    ParametricAssignee,
    Program,
    TransparentTypeDefinition,
    TupleExpression,
    TupleType,
    TypedAssignee,
    TypeInstance,
    TypeItem,
    UnionTypeDefinition,
    Var,
)
from grammar.GrammarLexer import GrammarLexer
from grammar.GrammarParser import GrammarParser
from grammar.GrammarVisitor import GrammarVisitor
from operators import Associativity, OperatorManager


class VisitorError(Exception): ...


class Visitor(GrammarVisitor):
    def visitList(self, ctx: ParserRuleContext) -> list:
        """Utility to visit a list of parse nodes (no type specified)."""
        children = (self.visit(child) for child in ctx.getChildren())
        return [child for child in children if child is not None]

    def visitId(self, ctx: GrammarParser.IdContext) -> str:
        return ctx.getText()

    def visitOperator_id(self, ctx) -> str:
        return re.match(r"^__(\S+)__$", ctx.getText()).group(1)

    def visitAtomic_type(self, ctx: GrammarParser.Atomic_typeContext) -> AtomicType:
        type_name = ctx.getText().upper()
        # Convert to `AtomicTypeEnum`
        type = AtomicTypeEnum[type_name]
        return AtomicType(type)

    def visitType_instance(self, ctx: GrammarParser.Type_instanceContext) -> TypeInstance:
        if ctx.type_instance() is not None:
            return self.visit(ctx.type_instance())
        return super().visitType_instance(ctx)

    def visitGeneric_list(self, ctx: GrammarParser.Generic_listContext) -> list[TypeInstance]:
        return self.visitList(ctx)

    def visitGeneric_type_instance(
        self, ctx: GrammarParser.Generic_type_instanceContext
    ) -> GenericType:
        generic_instance: GenericVariable = self.visit(ctx.generic_instance())
        return GenericType(generic_instance.id, generic_instance.type_instances)

    def visitGeneric_instance(self, ctx: GrammarParser.Generic_instanceContext) -> GenericVariable:
        if ctx.operator_id() is not None:
            id = self.visit(ctx.operator_id())
            return GenericVariable(id, [])
        id = self.visitId(ctx.id_())
        generic_list = [] if ctx.generic_list() is None else self.visit(ctx.generic_list())
        return GenericVariable(id, generic_list)

    def visitType_list(self, ctx: GrammarParser.Type_listContext) -> list[TypeInstance]:
        return self.visitList(ctx)

    def visitTuple_type(self, ctx: GrammarParser.Tuple_typeContext) -> TupleType:
        return TupleType(self.visit(ctx.type_list()))

    def visitFn_type_head(self, ctx: GrammarParser.Fn_type_headContext) -> list[TypeInstance]:
        if ctx.return_type() is not None:
            return_type = self.visit(ctx.return_type())
            if isinstance(return_type, TupleType):
                return return_type.types
            else:
                return [return_type]
        else:
            return [self.visit(ctx.type_instance())]

    def visitFn_type(self, ctx: GrammarParser.Fn_typeContext) -> FunctionType:
        argument_types = self.visit(ctx.fn_type_head())
        return_type = self.visit(ctx.fn_type_tail())
        return FunctionType(argument_types, return_type)

    def visitInteger(self, ctx: GrammarParser.IntegerContext) -> Integer:
        value = int(ctx.getText())
        return Integer(value)

    def visitBoolean(self, ctx: GrammarParser.BooleanContext) -> Boolean:
        if ctx.getText().lower() == "true":
            return Boolean(True)
        else:
            return Boolean(False)

    def visitNon_singleton_expr_list(
        self, ctx: GrammarParser.Non_singleton_expr_listContext
    ) -> list[Expression]:
        return self.visitList(ctx)

    def visitExpr_list(self, ctx: GrammarParser.Expr_listContext) -> list[Expression]:
        if ctx.expr() is None:
            return self.visit(ctx.non_singleton_expr_list())
        else:
            return [self.visit(ctx.expr())]

    def visitTuple_expr(self, ctx: GrammarParser.Tuple_exprContext) -> TupleExpression:
        expressions = self.visit(ctx.non_singleton_expr_list())
        return TupleExpression(expressions)

    def visitInfix_operator(self, ctx: GrammarParser.Infix_operatorContext) -> str:
        operator = ctx.getText().strip()
        match = re.match(r"^__(\S+)__$", ctx.getText())
        if match:
            operator = match.group(1)
        return operator

    def visitInfix_free_expr(self, ctx: GrammarParser.Infix_free_exprContext) -> Expression:
        if ctx.fn_call_free_expr() is None:
            return self.visit(ctx.fn_call())
        else:
            return self.visit(ctx.fn_call_free_expr())

    def visitInfix_call(self, ctx: GrammarParser.Infix_callContext, carry=None) -> FunctionCall:
        if carry is None:
            # Use a variable (highest precedence) as root (will eventually be ignored).
            carry = ("id", lambda x: x)

        left = self.visit(ctx.infix_free_expr())
        operator = self.visit(ctx.infix_operator())

        parent_operator, tree = carry
        if (
            operator == parent_operator
            and OperatorManager.get_associativity(operator) == Associativity.NONE
        ):
            raise VisitorError(f"{operator} is non-associative")

        if OperatorManager.get_precedence(parent_operator) < OperatorManager.get_precedence(
            operator
        ) or (
            operator == parent_operator
            and OperatorManager.get_associativity(operator) == Associativity.RIGHT
        ):
            # This operator has higher precedence, so make it the root of the new tree
            # and rotate the left subtree.
            carry = (
                operator,
                lambda x: FunctionCall(GenericVariable(operator, []), [tree(left), x]),
            )
        else:
            # This operator has lower precedence, so keep the parent as the root
            # and place at the base with the next argument.
            carry = (
                parent_operator,
                lambda x: tree(FunctionCall(GenericVariable(operator, []), [left, x])),
            )
        if ctx.expr().infix_call() is None:
            _, function = carry
            # Use the right node as the second argument.
            right = self.visit(ctx.expr())
            return function(right)
        else:
            # Continue parsing in the right subtree.
            return self.visitInfix_call(ctx.expr().infix_call(), carry=carry)

    def visitPrefix_call(self, ctx: GrammarParser.Prefix_callContext) -> FunctionCall:
        operator = self.visit(ctx.infix_operator())
        if not OperatorManager.check_operator(operator):
            raise VisitorError(f"Invalid prefix operator {operator}")
        argument = self.visit(ctx.expr())
        return FunctionCall(Var(operator), [argument])

    def visitFn_call(self, ctx: GrammarParser.Fn_callContext) -> FunctionCall:
        function = self.visit(ctx.fn_call_head())
        return self.visitFn_call_tail(ctx.fn_call_tail(), function=function)

    def visitAccess_tail(
        self, ctx: GrammarParser.Access_tailContext, expression=None
    ) -> ElementAccess:
        index = int(ctx.UINT().getText())
        access = ElementAccess(expression, index)
        if ctx.access_tail() is None:
            return access
        return self.visitAccess_tail(ctx.access_tail(), expression=access)

    def visitAccess(self, ctx: GrammarParser.AccessContext):
        expression = self.visit(ctx.access_head())
        return self.visitAccess_tail(ctx.access_tail(), expression=expression)

    def visitFn_call_access_free_expr(
        self, ctx: GrammarParser.Fn_call_access_free_exprContext
    ) -> Expression:
        if ctx.expr() is not None:
            return self.visit(ctx.expr())
        return super().visitFn_call_free_expr(ctx)

    def visitFn_call_tail(
        self, ctx: GrammarParser.Fn_call_tailContext, function=None
    ) -> FunctionCall:
        args = self.visit(ctx.expr_list())
        function = FunctionCall(function, args)
        if ctx.fn_call_tail() is not None:
            return self.visitFn_call_tail(ctx.fn_call_tail(), function)
        return function

    def visitId_list(self, ctx: GrammarParser.Id_listContext) -> list[str]:
        return self.visitList(ctx)

    def visitNon_generic_assignee(self, ctx: GrammarParser.Non_generic_assigneeContext) -> Assignee:
        return Assignee(ctx.getText())

    def visitGeneric_assignee(
        self, ctx: GrammarParser.Generic_assigneeContext
    ) -> ParametricAssignee:
        id = self.visit(ctx.non_generic_assignee())
        generics = [] if ctx.id_list() is None else self.visit(ctx.id_list())
        return ParametricAssignee(id, generics)

    def visitAssignee(self, ctx: GrammarParser.AssigneeContext) -> ParametricAssignee:
        if ctx.operator_id() is not None:
            id = self.visit(ctx.operator_id())
            return ParametricAssignee(Assignee(id), [])
        elif ctx.getText() == "__":
            # Handle edge case of the variable `__`.
            return ParametricAssignee(Assignee("__"), [])
        return super().visit(ctx.generic_assignee())

    def visitAssignment(self, ctx: GrammarParser.AssignmentContext) -> Assignment:
        assignee = self.visit(ctx.assignee())
        expression = self.visit(ctx.expr())
        return Assignment(assignee, expression)

    def visitAssignment_list(self, ctx: GrammarParser.Assignment_listContext) -> list[Assignee]:
        return self.visitList(ctx)

    def visitBlock(self, ctx: GrammarParser.BlockContext) -> Block:
        assignments = self.visit(ctx.assignment_list())
        expression = self.visit(ctx.expr())
        return Block(assignments, expression)

    def visitIf_expr(self, ctx: GrammarParser.If_exprContext) -> IfExpression:
        condition = self.visit(ctx.expr())
        true_ctx, false_ctx = ctx.block()
        true_block = self.visit(true_ctx)
        false_block = self.visit(false_ctx)
        return IfExpression(condition, true_block, false_block)

    def visitMatch_item(self, ctx: GrammarParser.Match_itemContext) -> MatchItem:
        name = self.visit(ctx.id_())
        assignee = (
            None if ctx.non_generic_assignee() is None else self.visit(ctx.non_generic_assignee())
        )
        return MatchItem(name, assignee)

    def visitMatch_list(self, ctx: GrammarParser.Match_listContext) -> list[MatchItem]:
        return self.visitList(ctx)

    def visitMatch_block(self, ctx: GrammarParser.Match_blockContext) -> MatchBlock:
        matches = self.visit(ctx.match_list())
        block = self.visit(ctx.block())
        return MatchBlock(matches, block)

    def visitMatch_block_list(self, ctx: GrammarParser.Match_block_listContext) -> list[MatchBlock]:
        return self.visitList(ctx)

    def visitMatch_expr(self, ctx: GrammarParser.Match_exprContext) -> MatchExpression:
        subject = self.visit(ctx.expr())
        blocks = self.visit(ctx.match_block_list())
        return MatchExpression(subject, blocks)

    def visitTyped_assignee(self, ctx: GrammarParser.Typed_assigneeContext) -> TypedAssignee:
        assignee = self.visit(ctx.non_generic_assignee())
        type_instance = self.visit(ctx.type_instance())
        return TypedAssignee(assignee, type_instance)

    def visitTyped_assignee_list(
        self, ctx: GrammarParser.Typed_assignee_listContext
    ) -> list[TypedAssignee]:
        return self.visitList(ctx)

    def visitFn_def(self, ctx: GrammarParser.Fn_defContext) -> FunctionDefinition:
        assignees = self.visit(ctx.typed_assignee_list())
        return_type = self.visit(ctx.type_instance())
        body = self.visit(ctx.block())
        return FunctionDefinition(assignees, return_type, body)

    def visitGeneric_constructor(
        self, ctx: GrammarParser.Generic_constructorContext
    ) -> GenericConstructor:
        generic_instance: GenericVariable = self.visit(ctx.generic_instance())
        return GenericConstructor(generic_instance.id, generic_instance.type_instances)

    def visitConstructor_call(self, ctx: GrammarParser.Constructor_callContext) -> ConstructorCall:
        constructor: GenericConstructor = self.visit(ctx.generic_constructor())
        if OperatorManager.check_operator(constructor.id):
            raise VisitorError(f"Invalid constructor id {constructor.id}")
        arguments = self.visit(ctx.expr_list())
        return ConstructorCall(constructor, arguments)

    def visitGeneric_typevar(
        self, ctx: GrammarParser.Generic_typevarContext
    ) -> GenericTypeVariable:
        assignee: Assignee = self.visit(ctx.generic_assignee())
        return GenericTypeVariable(assignee.assignee.id, assignee.generic_variables)

    def visitType_item(self, ctx: GrammarParser.Type_itemContext) -> TypeItem:
        id = self.visit(ctx.id_())
        type_instance = None if ctx.type_instance() is None else self.visit(ctx.type_instance())
        return TypeItem(id, type_instance)

    def visitUnion_def(self, ctx: GrammarParser.Union_defContext) -> list[TypeItem]:
        return self.visitList(ctx)

    def visitType_def(
        self, ctx: GrammarParser.Type_defContext
    ) -> EmptyTypeDefinition | OpaqueTypeDefinition | UnionTypeDefinition:
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

    def visitType_alias(self, ctx: GrammarParser.Type_aliasContext) -> TransparentTypeDefinition:
        type_variable = self.visit(ctx.generic_typevar())
        type_instance = self.visit(ctx.type_instance())
        return TransparentTypeDefinition(type_variable, type_instance)

    def visitDefinitions(self, ctx: GrammarParser.DefinitionsContext) -> list[Definition]:
        return self.visitList(ctx)

    def visitProgram(self, ctx: GrammarParser.ProgramContext) -> Program:
        definitions = self.visit(ctx.definitions())
        return Program(definitions)


class Parser:
    @staticmethod
    def parse(code: str, target: str) -> Optional[ASTNode]:
        input_stream = InputStream(code)
        lexer = GrammarLexer(input_stream)
        stream = CommonTokenStream(lexer)
        parser = GrammarParser(stream)
        if target in parser.ruleNames:
            tree = getattr(parser, target).__call__()
            # Require no errors and at the end of the file.
            if parser.getNumberOfSyntaxErrors() > 0 or stream.LA(1) != Token.EOF:
                return None
            visitor = Visitor()
            try:
                # Fail if there are any errors converting parse tree to AST.
                return visitor.visit(tree)
            except VisitorError:
                return None
        return None
