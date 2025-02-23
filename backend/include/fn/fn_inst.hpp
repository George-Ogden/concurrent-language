#pragma once

#include "lazy/types.hpp"
#include "types/compound.hpp"

#include <memory>
#include <type_traits>

template <typename Ret, typename... Args> class TypedFnI {
  protected:
    using ArgsT = LazyT<TupleT<std::decay_t<Args>...>>;
    using RetT = LazyT<std::decay_t<Ret>>;
    ArgsT args;
    virtual RetT
    body(std::add_lvalue_reference_t<LazyT<std::decay_t<Args>>>...) = 0;

  public:
    TypedFnI();
    virtual ~TypedFnI();
    explicit TypedFnI(const ArgsT &);
    RetT run();
};

template <typename E, typename Ret, typename... Args>
struct TypedClosureI : public TypedFnI<Ret, Args...> {
    using typename TypedFnI<Ret, Args...>::ArgsT;
    using typename TypedFnI<Ret, Args...>::RetT;
    using EnvT = LazyT<E>;
    using TypedFnI<Ret, Args...>::TypedFnI;
    TypedClosureI(const ArgsT &, EnvT);

  protected:
    EnvT env;
};

template <typename Ret, typename... Args>
struct TypedClosureI<Empty, Ret, Args...> : public TypedFnI<Ret, Args...> {
    using typename TypedFnI<Ret, Args...>::ArgsT;
    using typename TypedFnI<Ret, Args...>::RetT;
    using TypedFnI<Ret, Args...>::TypedFnI;
};
