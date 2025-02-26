#pragma once

#include "fn/fn_inst.hpp"
#include "lazy/types.hpp"
#include "types/builtin.hpp"
#include "types/compound.hpp"

#include <memory>
#include <type_traits>

template <typename Ret, typename... Args> struct WeakTypedFnG;

template <typename Ret, typename... Args> struct TypedFnG {
    friend class WeakTypedFnG<Ret, Args...>;
    using RetT = LazyT<std::decay_t<Ret>>;
    using ArgsT = LazyT<TupleT<std::decay_t<Args>...>>;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(
        const ArgsT &, std::shared_ptr<void>);
    using U = std::unique_ptr<TypedFnI<Ret, Args...>>;
    T _fn = nullptr;
    std::shared_ptr<void> _env;
    TypedFnG(T fn, const std::shared_ptr<void> env);
    explicit TypedFnG(T fn);
    TypedFnG();
    virtual ~TypedFnG();
    virtual U init(LazyT<std::decay_t<Args>>... args) const;
    explicit TypedFnG(WeakTypedFnG<Ret, Args...> f);
};

template <typename Ret, typename... Args> struct WeakTypedFnG {
    friend class TypedFnG<Ret, Args...>;
    using RetT = typename TypedFnG<Ret, Args...>::RetT;
    using ArgsT = typename TypedFnG<Ret, Args...>::ArgsT;
    using T = typename TypedFnG<Ret, Args...>::T;
    using U = std::unique_ptr<TypedFnI<Ret, Args...>>;
    T _fn = nullptr;
    std::weak_ptr<void> _env;
    WeakTypedFnG();
    explicit WeakTypedFnG(TypedFnG<Ret, Args...> f);
};

template <typename E, typename Ret, typename... Args>
struct TypedClosureG : public TypedFnG<Ret, Args...> {
    using typename TypedFnG<Ret, Args...>::ArgsT;
    using typename TypedFnG<Ret, Args...>::RetT;
    using EnvT = LazyT<E>;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(
        const ArgsT &, std::shared_ptr<EnvT>);
    using TypedFnG<Ret, Args...>::U;
    TypedClosureG(T fn, EnvT env);
    explicit TypedClosureG(T fn);
    EnvT &env();
};

template <typename Ret, typename... Args>
struct TypedClosureG<Empty, Ret, Args...> : public TypedFnG<Ret, Args...> {
    using typename TypedFnG<Ret, Args...>::ArgsT;
    using typename TypedFnG<Ret, Args...>::RetT;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(const ArgsT &);
    using TypedFnG<Ret, Args...>::U;
    using TypedFnG<Ret, Args...>::TypedFnG;
};
