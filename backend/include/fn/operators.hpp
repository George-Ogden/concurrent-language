#pragma once

#include "fn/fn_gen.tpp"
#include "fn/fn_inst.tpp"
#include "lazy/types.hpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <compare>
#include <memory>

// Macros to turn functions into function generators.
#define Binary_Int_Int_Int_Op__BuiltIn(fn, size)                               \
    class fn##_I : public TypedFnI<Int, Int, Int> {                            \
      protected:                                                               \
        LazyT<Int> body(LazyT<Int> &x, LazyT<Int> &y) override {               \
            WorkManager::enqueue(x);                                           \
            WorkManager::enqueue(y);                                           \
            WorkManager::await(x, y);                                          \
            return make_lazy<Int>(fn(x->value(), y->value()));                 \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Int, Int, Int>::TypedFnI;                               \
        static std::unique_ptr<TypedFnI<Int, Int, Int>>                        \
        init(const ArgsT &args) {                                              \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr bool is_recursive() const override { return false; };        \
    };                                                                         \
    FnT<Int, Int, Int> fn##_G =                                                \
        std::make_shared<TypedClosureG<Empty, Int, Int, Int>>(fn##_I::init);

#define Unary_Int_Int_Op__BuiltIn(fn, size)                                    \
    class fn##_I : public TypedFnI<Int, Int> {                                 \
      protected:                                                               \
        LazyT<Int> body(LazyT<Int> &x) override {                              \
            WorkManager::enqueue(x);                                           \
            WorkManager::await(x);                                             \
            return make_lazy<Int>(fn(x->value()));                             \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Int, Int>::TypedFnI;                                    \
        static std::unique_ptr<TypedFnI<Int, Int>> init(const ArgsT &args) {   \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr bool is_recursive() const override { return false; };        \
    };                                                                         \
    FnT<Int, Int> fn##_G =                                                     \
        std::make_shared<TypedClosureG<Empty, Int, Int>>(fn##_I::init);

#define Unary_Bool_Bool_Op__BuiltIn(fn, size)                                  \
    class fn##_I : public TypedFnI<Bool, Bool> {                               \
      protected:                                                               \
        LazyT<Bool> body(LazyT<Bool> &x) override {                            \
            WorkManager::enqueue(x);                                           \
            WorkManager::await(x);                                             \
            return make_lazy<Bool>(fn(x->value()));                            \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Bool, Bool>::TypedFnI;                                  \
        static std::unique_ptr<TypedFnI<Bool, Bool>> init(const ArgsT &args) { \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr bool is_recursive() const override { return false; };        \
    };                                                                         \
    FnT<Bool, Bool> fn##_G =                                                   \
        std::make_shared<TypedClosureG<Empty, Bool, Bool>>(fn##_I::init);

#define Binary_Int_Int_Bool_Op__BuiltIn(fn, size)                              \
    class fn##_I : public TypedFnI<Bool, Int, Int> {                           \
      protected:                                                               \
        LazyT<Bool> body(LazyT<Int> &x, LazyT<Int> &y) override {              \
            WorkManager::enqueue(x);                                           \
            WorkManager::enqueue(y);                                           \
            WorkManager::await(x, y);                                          \
            return make_lazy<Bool>(fn(x->value(), y->value()));                \
        }                                                                      \
                                                                               \
      public:                                                                  \
        using TypedFnI<Bool, Int, Int>::TypedFnI;                              \
        static std::unique_ptr<TypedFnI<Bool, Int, Int>>                       \
        init(const ArgsT &args) {                                              \
            return std::make_unique<fn##_I>(args);                             \
        }                                                                      \
        constexpr std::size_t lower_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr std::size_t upper_size_bound() const override {              \
            return size;                                                       \
        };                                                                     \
        constexpr bool is_recursive() const override { return false; };        \
    };                                                                         \
    FnT<Bool, Int, Int> fn##_G =                                               \
        std::make_shared<TypedClosureG<Empty, Bool, Int, Int>>(fn##_I::init);

// Operator definitions.
Int Plus__BuiltIn(Int x, Int y) { return x + y; }

Int Minus__BuiltIn(Int x, Int y) { return x - y; }

Int Multiply__BuiltIn(Int x, Int y) { return x * y; }

Int Divide__BuiltIn(Int x, Int y) { return x / y; }

Int Exponentiate__BuiltIn(Int x, Int y) {
    if (y < 0)
        return 0;
    Int res = 1, base = x, exp = y;
    while (exp) {
        if (exp & 1)
            res *= base;
        exp >>= 1;
        base *= base;
    }
    return res;
}

Int Modulo__BuiltIn(Int x, Int y) { return x % y; }

Int Right_Shift__BuiltIn(Int x, Int y) { return x >> y; }

Int Left_Shift__BuiltIn(Int x, Int y) { return x << y; }

Int Spaceship__BuiltIn(Int x, Int y) {
    const auto o = (x <=> y);
    if (o == std::strong_ordering::less)
        return -1;
    if (o == std::strong_ordering::greater)
        return 1;
    return 0;
}

Int Bitwise_And__BuiltIn(Int x, Int y) { return x & y; }

Int Bitwise_Or__BuiltIn(Int x, Int y) { return x | y; }

Int Bitwise_Xor__BuiltIn(Int x, Int y) { return x ^ y; }

Int Increment__BuiltIn(Int x) { return x + 1; }

Int Decrement__BuiltIn(Int x) { return x - 1; }

Bool Negation__BuiltIn(Bool x) { return !x; }

Bool Comparison_LT__BuiltIn(Int x, Int y) { return x < y; }

Bool Comparison_LE__BuiltIn(Int x, Int y) { return x <= y; }

Bool Comparison_EQ__BuiltIn(Int x, Int y) { return x == y; }

Bool Comparison_NE__BuiltIn(Int x, Int y) { return x != y; }

Bool Comparison_GT__BuiltIn(Int x, Int y) { return x > y; }

Bool Comparison_GE__BuiltIn(Int x, Int y) { return x >= y; }

Binary_Int_Int_Int_Op__BuiltIn(Plus__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Minus__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Multiply__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Divide__BuiltIn, 10);
Binary_Int_Int_Int_Op__BuiltIn(Exponentiate__BuiltIn, 12);
Binary_Int_Int_Int_Op__BuiltIn(Modulo__BuiltIn, 10);
Binary_Int_Int_Int_Op__BuiltIn(Right_Shift__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Left_Shift__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Spaceship__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_And__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Or__BuiltIn, 9);
Binary_Int_Int_Int_Op__BuiltIn(Bitwise_Xor__BuiltIn, 9);

Unary_Int_Int_Op__BuiltIn(Increment__BuiltIn, 8);
Unary_Int_Int_Op__BuiltIn(Decrement__BuiltIn, 8);

Unary_Bool_Bool_Op__BuiltIn(Negation__BuiltIn, 8);

Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LT__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_LE__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_EQ__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_NE__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GT__BuiltIn, 9);
Binary_Int_Int_Bool_Op__BuiltIn(Comparison_GE__BuiltIn, 9);
