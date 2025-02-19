#pragma once

#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "fn/types.hpp"
#include "lazy/types.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"
#include "work/status.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>

class Work {
  protected:
    Locked<std::vector<Continuation>> continuations;
    template <typename T, typename U> static void assign(T &targets, U &result);

  public:
    std::atomic<Status> status;
    Work();
    virtual ~Work();
    virtual void run() = 0;
    virtual void await_all() = 0;
    bool done() const;
    template <typename Ret, typename... Args>
    static std::pair<std::shared_ptr<Work>, Ret>
    fn_call(TypedFn<Ret, Args...> f, Args... args);
    void add_continuation(Continuation c);
};

using WorkT = std::shared_ptr<Work>;

template <typename Ret, typename... Args> class TypedWork : public Work {
    friend class Work;
    WeakLazyPlaceholdersT<Ret> targets;
    LazyT<TupleT<Args...>> args;
    FnT<Ret, Args...> fn;

  public:
    void run() override;
    void await_all() override;
};

struct FinishWork : public Work {
    FinishWork();
    void run() override;
    void await_all() override;
};
