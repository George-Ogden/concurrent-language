#pragma once

#include "data_structures/atomic_shared_enum.hpp"
#include "data_structures/lock.hpp"
#include "fn/fn_inst.hpp"
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
    enum WorkStatus { AVAILABLE, ACTIVE, DONE, MAX };
    template <typename T, typename U> static void assign(T &targets, U &result);
    constexpr static inline unsigned ATOMIC_WIDTH = 2;
    AtomicSharedEnum<ATOMIC_WIDTH> work_status;
    static_assert(MAX <= (1ULL << ATOMIC_WIDTH));

  public:
    Work();
    virtual ~Work();
    virtual void run() = 0;
    virtual void await_all() = 0;
    bool done() const;
    void finish();
    template <typename Ret, typename... Args, typename... ArgsT>
    requires(std::is_same_v<Args, remove_lazy_t<std::decay_t<ArgsT>>>
                 &&...) static std::
        pair<std::shared_ptr<Work>, LazyT<Ret>> fn_call(
            const FnT<Ret, Args...> &f, const ArgsT &...args);
    virtual bool can_respond() const = 0;
};

using WorkT = std::shared_ptr<Work>;
using WeakWorkT = std::weak_ptr<Work>;

template <typename Ret, typename... Args> class TypedWork : public Work {
    friend class Work;

  protected:
    WeakLazyPlaceholdersT<Ret> targets;
    std::unique_ptr<TypedFnI<Ret, Args...>> fn;

  public:
    void run() override;
    void await_all() override;
    bool can_respond() const override;
};
