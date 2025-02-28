#pragma once

#include "fn/fn_inst.hpp"
#include "lazy/types.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"

#include <memory>
#include <type_traits>

template <typename Ret, typename... Args> struct TypedFnG {
    using RetT = LazyT<std::decay_t<Ret>>;
    using ArgsT = LazyT<TupleT<std::decay_t<Args>...>>;
    using Fn = TypedFnG<Ret, std::decay_t<Args>...>;
    using U = std::unique_ptr<TypedFnI<Ret, Args...>>;
    TypedFnG();
    virtual ~TypedFnG();
    virtual U init(LazyT<std::decay_t<Args>>... args) const = 0;
};

template <typename E, typename Ret, typename... Args>
struct TypedClosureG : public TypedFnG<Ret, Args...> {
    using typename TypedFnG<Ret, Args...>::ArgsT;
    using typename TypedFnG<Ret, Args...>::RetT;
    using typename TypedFnG<Ret, Args...>::Fn;
    using EnvT = LazyT<E>;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(const ArgsT &,
                                                          const EnvT &);
    using typename TypedFnG<Ret, Args...>::U;
    T fn;
    EnvT env;
    TypedClosureG(T fn, const EnvT &env);
    explicit TypedClosureG(T fn);
    TypedClosureG();
    U init(LazyT<std::decay_t<Args>>... args) const override;
};

template <typename Ret, typename... Args>
struct TypedClosureG<Empty, Ret, Args...> : public TypedFnG<Ret, Args...> {
    using typename TypedFnG<Ret, Args...>::ArgsT;
    using typename TypedFnG<Ret, Args...>::RetT;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(const ArgsT &);
    using typename TypedFnG<Ret, Args...>::Fn;
    using typename TypedFnG<Ret, Args...>::U;
    using TypedFnG<Ret, Args...>::TypedFnG;
    T fn;
    explicit TypedClosureG(T fn);
    TypedClosureG();
    U init(LazyT<std::decay_t<Args>>... args) const override;
};
