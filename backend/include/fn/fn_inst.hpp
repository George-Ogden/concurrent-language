#pragma once

#include "lazy/types.hpp"
#include "types/compound.hpp"

#include <memory>
#include <type_traits>

template <typename Ret, typename... Args> struct TypedFnG;
template <typename Ret, typename... Args> class TypedFnI {
  protected:
    using ArgsT = LazyT<TupleT<std::decay_t<Args>...>>;
    using RetT = LazyT<std::decay_t<Ret>>;
    using Fn = TypedFnG<Ret, std::decay_t<Args>...>;
    ArgsT args;
    virtual RetT
    body(std::add_lvalue_reference_t<LazyT<std::decay_t<Args>>>...) = 0;

  public:
    TypedFnI();
    virtual ~TypedFnI();
    explicit TypedFnI(const ArgsT &);
    RetT run();
    virtual void set_fn(const std::shared_ptr<TypedFnG<Ret, Args...>> &fn);
    virtual constexpr std::size_t lower_size_bound() const = 0;
    virtual constexpr std::size_t upper_size_bound() const = 0;
};

template <typename E, typename Ret, typename... Args>
struct TypedClosureI : public TypedFnI<Ret, Args...> {
    using typename TypedFnI<Ret, Args...>::ArgsT;
    using typename TypedFnI<Ret, Args...>::RetT;
    using typename TypedFnI<Ret, Args...>::Fn;
    using EnvT = LazyT<E>;
    using TypedFnI<Ret, Args...>::TypedFnI;
    TypedClosureI(const ArgsT &, const EnvT &);

  protected:
    EnvT env;
    std::shared_ptr<TypedFnG<Ret, Args...>> fn;
    void set_fn(const std::shared_ptr<TypedFnG<Ret, Args...>> &fn) override;
};

template <typename Ret, typename... Args>
struct TypedClosureI<Empty, Ret, Args...> : public TypedFnI<Ret, Args...> {
    using typename TypedFnI<Ret, Args...>::ArgsT;
    using typename TypedFnI<Ret, Args...>::RetT;
    using typename TypedFnI<Ret, Args...>::Fn;
    using TypedFnI<Ret, Args...>::TypedFnI;
};
