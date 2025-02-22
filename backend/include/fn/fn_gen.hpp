#pragma once

#include "fn/fn_inst.hpp"
#include "lazy/types.hpp"
#include "types/compound.hpp"

#include <memory>
#include <type_traits>

class FnG {
  protected:
    void *_fn = nullptr;
    std::shared_ptr<void> _env;
    FnG(void *fn, std::shared_ptr<void> env);
    explicit FnG(void *fn);

  public:
    FnG();
    virtual ~FnG();
};

template <typename Ret, typename... Args> struct TypedFnG : public FnG {
    using RetT = LazyT<Ret>;
    using ArgsT = LazyT<TupleT<Args...>>;
    using T = std::unique_ptr<TypedFnI<Ret, Args...>> (*)(
        const ArgsT &, std::shared_ptr<void>);
    using U = std::unique_ptr<TypedFnI<Ret, Args...>>;
    TypedFnG(T fn, const std::shared_ptr<void> env);
    explicit TypedFnG(T fn);
    TypedFnG();
    virtual U init(LazyT<Args>... args) const;
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
