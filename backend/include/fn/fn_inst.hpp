#pragma once

#include "lazy/types.hpp"
#include "types/compound.hpp"

#include <cstdint>
#include <map>
#include <memory>
#include <type_traits>
#include <variant>
#include <vector>

class Work;
static const inline std::size_t IMMEDIATE_EXECUTION_THRESHOLD = 50;
using MapVariantT = std::variant<Int, void *>;

template <typename Ret, typename... Args> struct TypedFnG;
template <typename Ret, typename... Args> class TypedFnI {

  protected:
    using ArgsT = LazyT<TupleT<std::decay_t<Args>...>>;
    using RetT = LazyT<std::decay_t<Ret>>;
    using Fn = TypedFnG<Ret, std::decay_t<Args>...>;
    ArgsT args;
    virtual RetT
    body(std::add_lvalue_reference_t<LazyT<std::decay_t<Args>>>...) = 0;
    std::map<std::vector<MapVariantT>, std::shared_ptr<LazyValue>> cache;

  public:
    TypedFnI();
    virtual ~TypedFnI();
    explicit TypedFnI(const ArgsT &);
    RetT run();
    virtual void set_fn(const std::shared_ptr<TypedFnG<Ret, Args...>> &fn);
    void process(std::shared_ptr<Work> &work) const;
    template <typename R, typename... As, typename... AT>
    requires(std::is_same_v<As, remove_lazy_t<std::decay_t<AT>>> &&...)
        LazyT<R> fn_call(const std::shared_ptr<TypedFnG<R, As...>> &f,
                         const AT &...args);
    virtual constexpr std::size_t lower_size_bound() const = 0;
    virtual constexpr std::size_t upper_size_bound() const = 0;
    virtual constexpr bool is_recursive() const = 0;
    constexpr bool execute_immediately() const;
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
