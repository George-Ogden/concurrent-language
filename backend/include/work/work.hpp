#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "fn/fn_inst.hpp"
#include "fn/types.hpp"
#include "lazy/types.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"
#include "work/status.hpp"

#include <atomic>
#include <initializer_list>
#include <memory>
#include <utility>
#include <vector>

class Work {
  protected:
    Locked<std::vector<Continuation>> continuations;
    Locked<std::vector<std::weak_ptr<LazyValue>>> dependencies;
    template <typename T, typename U> static void assign(T &targets, U &result);

  public:
    Status status;
    Work();
    virtual ~Work();
    virtual void run() = 0;
    virtual void await_all() = 0;
    bool done() const;
    template <typename Ret, typename... Args, typename... ArgsT>
    requires(
        std::is_same_v<Args, remove_lazy_t<std::decay_t<ArgsT>>>
            &&...) static std::pair<std::shared_ptr<Work>,
                                    LazyT<Ret>> fn_call(FnT<Ret, Args...> f,
                                                        ArgsT... args);
    void add_continuation(Continuation c);
    void add_dependencies(
        std::initializer_list<std::shared_ptr<LazyValue>> &&dependencies);
};

using WorkT = std::shared_ptr<Work>;
using WeakWorkT = std::weak_ptr<Work>;

template <typename Ret, typename... Args> class TypedWork : public Work {
    friend class Work;
    WeakLazyPlaceholdersT<Ret> targets;
    std::unique_ptr<TypedFnI<Ret, Args...>> fn;

  public:
    void run() override;
    void await_all() override;
};
