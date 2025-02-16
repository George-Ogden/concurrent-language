#pragma once

#include "data_structures/lazy.hpp"
#include "data_structures/lock.hpp"
#include "fn/continuation.hpp"
#include "types/compound.hpp"
#include "types/utils.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>

class Work {
  protected:
    std::atomic<bool> done_flag;
    Locked<std::vector<Continuation>> continuations;
    template <typename T, typename U> static void assign(T &targets, U &result);

  public:
    virtual void run() = 0;
    virtual ~Work();
    bool done() const;
    template <typename Ret, typename... Args>
    static std::pair<std::shared_ptr<Work>, LazyT<TupleT<Args...>>>
    fn_call(TypedFn<Ret, Args...> f, Args... args);
    void add_continuation(Continuation c);
    static void update_continuation(Continuation c);
};

template <typename Ret, typename... Args> class TypedWork : public Work {
    friend class Work;
    WeakLazyT<Ret> targets;
    LazyT<TupleT<Args...>> args;
    TypedFn<LazyT<Ret>, LazyT<Args>...> fn;

  public:
    void run() override;
};
