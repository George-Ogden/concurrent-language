#pragma once

#include "data_structures/lazy.tpp"
#include "fn/fn.tpp"
#include "system/work_manager.tpp"
#include "types/builtin.hpp"
#include "types/utils.hpp"

#include <compare>

LazyT<Int> Plus__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                         std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() + y->value());
}

LazyT<Int> Minus__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                          std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() - y->value());
}

LazyT<Int> Multiply__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                             std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() * y->value());
}

LazyT<Int> Divide__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                           std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() / y->value());
}

LazyT<Int> Exponentiate__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                 std::shared_ptr<void> env = nullptr) {
    WorkManager::await(y);
    if (y->value() < 0)
        return make_lazy<Int>(0);
    WorkManager::await(x);
    Int res = 1, base = x->value(), exp = y->value();
    while (exp) {
        if (exp & 1)
            res *= base;
        exp >>= 1;
        base *= base;
    }
    return make_lazy<Int>(res);
}

LazyT<Int> Modulo__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                           std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() % y->value());
}

LazyT<Int> Right_Shift__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() >> y->value());
}

LazyT<Int> Left_Shift__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                               std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() << y->value());
}

LazyT<Int> Spaceship__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                              std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    const auto o = (x->value() <=> y->value());
    if (o == std::strong_ordering::less)
        return make_lazy<Int>(-1);
    if (o == std::strong_ordering::greater)
        return make_lazy<Int>(1);
    return make_lazy<Int>(0);
}

LazyT<Int> Bitwise_And__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() & y->value());
}

LazyT<Int> Bitwise_Or__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                               std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() | y->value());
}

LazyT<Int> Bitwise_Xor__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Int>(x->value() ^ y->value());
}

LazyT<Int> Increment__BuiltIn(LazyT<Int> x,
                              std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x);
    return make_lazy<Int>(x->value() + 1);
}

LazyT<Int> Decrement__BuiltIn(LazyT<Int> x,
                              std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x);
    return make_lazy<Int>(x->value() - 1);
}

LazyT<Bool> Negation__BuiltIn(LazyT<Bool> x,
                              std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x);
    return make_lazy<Bool>(!x->value());
}

LazyT<Bool> Comparison_LT__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() < y->value());
}

LazyT<Bool> Comparison_LE__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() <= y->value());
}

LazyT<Bool> Comparison_EQ__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() == y->value());
}

LazyT<Bool> Comparison_NE__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() != y->value());
}

LazyT<Bool> Comparison_GT__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() > y->value());
}

LazyT<Bool> Comparison_GE__BuiltIn(LazyT<Int> x, LazyT<Int> y,
                                   std::shared_ptr<void> env = nullptr) {
    WorkManager::await(x, y);
    return make_lazy<Bool>(x->value() >= y->value());
}

FnT<Int, Int, Int> Plus__BuiltIn_Fn{Plus__BuiltIn};
FnT<Int, Int, Int> Minus__BuiltIn_Fn{Minus__BuiltIn};
FnT<Int, Int, Int> Multiply__BuiltIn_Fn{Multiply__BuiltIn};
FnT<Int, Int, Int> Divide__BuiltIn_Fn{Divide__BuiltIn};
FnT<Int, Int, Int> Exponentiate__BuiltIn_Fn{Exponentiate__BuiltIn};
FnT<Int, Int, Int> Modulo__BuiltIn_Fn{Modulo__BuiltIn};
FnT<Int, Int, Int> Right_Shift__BuiltIn_Fn{Right_Shift__BuiltIn};
FnT<Int, Int, Int> Left_Shift__BuiltIn_Fn{Left_Shift__BuiltIn};
FnT<Int, Int, Int> Spaceship__BuiltIn_Fn{Spaceship__BuiltIn};
FnT<Int, Int, Int> Bitwise_And__BuiltIn_Fn{Bitwise_And__BuiltIn};
FnT<Int, Int, Int> Bitwise_Or__BuiltIn_Fn{Bitwise_Or__BuiltIn};
FnT<Int, Int, Int> Bitwise_Xor__BuiltIn_Fn{Bitwise_Xor__BuiltIn};

FnT<Int, Int> Increment__BuiltIn_Fn{Increment__BuiltIn};
FnT<Int, Int> Decrement__BuiltIn_Fn{Decrement__BuiltIn};

FnT<Bool, Bool> Negation__BuiltIn_Fn{Negation__BuiltIn};

FnT<Bool, Int, Int> Comparison_LT__BuiltIn_Fn{Comparison_LT__BuiltIn};
FnT<Bool, Int, Int> Comparison_LE__BuiltIn_Fn{Comparison_LE__BuiltIn};
FnT<Bool, Int, Int> Comparison_EQ__BuiltIn_Fn{Comparison_EQ__BuiltIn};
FnT<Bool, Int, Int> Comparison_NE__BuiltIn_Fn{Comparison_NE__BuiltIn};
FnT<Bool, Int, Int> Comparison_GT__BuiltIn_Fn{Comparison_GT__BuiltIn};
FnT<Bool, Int, Int> Comparison_GE__BuiltIn_Fn{Comparison_GE__BuiltIn};
