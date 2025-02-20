#pragma once

#include "fn/fn.tpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.hpp"
#include "lazy/lazy.tpp"
#include "lazy/types.hpp"
#include "lazy/fns.hpp"
#include "work/work.tpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>
#include <chrono>
#include <deque>

using namespace std::chrono_literals;

WorkT WorkManager::finish_work = std::make_shared<FinishWork>();

template <typename Ret, typename... Args>
Ret WorkManager::run(TypedFn<Ret, Args...> fn, Args...args)
{
    auto [work, result] = Work::fn_call(fn, args...);
    std::atomic<WorkT> ref{work};
    ThreadManager::RunConfig config{ThreadManager::available_concurrency(),
                                    false};
    WorkManager::work_queue->clear();
    WorkManager::counters = std::vector<std::atomic<unsigned>>(
        ThreadManager::available_concurrency());
    WorkManager::private_work_stacks = decltype(private_work_stacks)(
        ThreadManager::available_concurrency());
    ThreadManager::run_multithreaded(main, &ref, config);
    WorkManager::work_queue->clear();
    return result;
}

void WorkManager::enqueue(WorkT work)
{
    if (dynamic_cast<FinishWork*>(work.get()) != nullptr || work->status.enqueue()){
        WorkManager::work_queue.acquire();
        WorkManager::work_queue->push_back(work);
        WorkManager::work_queue.release();
    }
}

void WorkManager::try_priority_enqueue(WorkT work)
{
    if (work->status.require()){
        priority_enqueue(work);
    }
}

void WorkManager::priority_enqueue(WorkT work)
{
    auto id = ThreadManager::get_id();
    WorkManager::private_work_stacks[id].acquire();
    WorkManager::private_work_stacks[id]->push_back(work);
    WorkManager::private_work_stacks[id].release();
}

std::monostate WorkManager::main(std::atomic<WorkT> *ref)
{
    {
        WorkT work = ref->exchange(nullptr, std::memory_order_relaxed);
        if (work != nullptr)
        {
            work->run();
            work->await_all();
            enqueue(WorkManager::finish_work);
        }
        else
        {
            while (1)
            {
                std::tie(work, std::ignore) = get_work();
                if (work == nullptr)
                {
                    continue;
                }
                if (dynamic_cast<FinishWork *>(work.get()) != nullptr)
                {
                    enqueue(work);
                    break;
                }
                try
                {
                    work->run();
                }
                catch (finished &e)
                {
                    break;
                }
            }
        }
    }
    return std::monostate{};
}

std::pair<WorkT,bool> WorkManager::get_work()
{
    auto id = ThreadManager::get_id();
    Locked<std::deque<WorkT>> &stack = WorkManager::private_work_stacks[id];
    stack.acquire();
    while (!stack->empty())
    {
        WorkT work = stack->back();
        stack->pop_back();
        if (work != nullptr){
            stack.release();
            return std::make_pair(work, true);
        }
    }
    stack.release();

    Locked<std::deque<WeakWorkT>> &queue = WorkManager::work_queue;
    queue.acquire();
    while (!queue->empty()) {
        WorkT work = queue->front().lock();
        queue->pop_front();
        if (work != nullptr && (dynamic_cast<FinishWork*>(work.get()) != nullptr || work->status.dequeue())){
            queue.release();
            return std::make_pair(work, false);
        }
    }
    queue.release();
    return std::make_pair(nullptr, false);
}
template <typename T>
constexpr auto filter_awaitable(T &v)
{
    return std::tuple<std::decay_t<T>>(v);
}

template <typename... Ts>
constexpr auto filter_awaitable(std::tuple<Ts...> &v)
{
    return std::tuple<>{};
}

template <typename... Vs>
void WorkManager::await(Vs &...vs)
{
    std::apply([&](auto &&...ts)
               { await_restricted(ts...); },
               std::tuple_cat(filter_awaitable(vs)...));
}

template <typename T>
void await_variants(T &v) {
}

template <typename... Ts>
void await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l)
{
    auto v = l->value();
    std::size_t idx = v.tag;
    using AwaitWork = void (*)(std::aligned_union_t<0, Ts...> &);

    static constexpr AwaitWork waiters[sizeof...(Ts)] = {[](auto &storage)
                                                         {
                                                             WorkManager::await_all(
                                                                 std::launder(reinterpret_cast<Ts *>(&storage))->value);
                                                         }...};

    waiters[idx](v.value);
}

template <typename... Vs>
void WorkManager::await_all(Vs &...vs)
{
    if constexpr (sizeof...(vs) != 0)
    {
        auto flat_types = flatten(std::make_tuple(vs...));
        std::apply([&](auto &&...ts)
                   { await_restricted(ts...); }, flat_types);
        std::apply([&](auto &&...ts)
                   { (await_variants(ts), ...); },
                   flat_types);
    }
}

template <typename... Vs>
void WorkManager::await_restricted(Vs &...vs)
{
    unsigned n = sizeof...(vs);
    if (n == 0)
    {
        return;
    }
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{n};
    std::atomic<unsigned> &counter = counters[ThreadManager::get_id()];
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    (vs->add_continuation(c), ...);
    if (all_done(vs...))
    {
        return exit_early(c);
    }
    std::vector<WorkT> required_work;
    required_work.reserve(sizeof...(vs));
    (vs->save_work(required_work), ...);
    for (WorkT& work: required_work){
        try_priority_enqueue(work);
    }
    while (true)
    {
        auto [work, high_priority] = get_work();
        if (dynamic_cast<FinishWork *>(work.get()) != nullptr)
        {
            enqueue(work);
            throw finished{};
        }
        if (break_on_work(std::make_pair(work, high_priority), c)){
            while (!all_done(vs...)){}
            return;
        }
    }
};

void WorkManager::exit_early(Continuation &c){
    delete c.valid;
    if (c.counter.fetch_sub(1, std::memory_order_relaxed) == 1)
    {
        return;
    }
    else
    {
        throw stack_inversion{};
    }
}

bool WorkManager::break_on_work(std::pair<WorkT,bool> work_pair, Continuation &c){
    auto &[work, high_priority] = work_pair;
    try {
        if (c.counter.load(std::memory_order_relaxed) > 0) {
            throw stack_inversion{};
        }
        if (work != nullptr) {
            work->run();
        }
    } catch (stack_inversion &e) {
        c.valid->acquire();
        bool was_valid = **c.valid;
        **c.valid = false;
        c.valid->release();
        if (work != nullptr && !work->done()) {
            work->status.cancel_work();
            if (high_priority){
                priority_enqueue(work);
            } else {
                enqueue(work);
            }
        }
        if (!was_valid) {
            delete c.valid;
        }
        c.counter.fetch_sub(1 - was_valid, std::memory_order_relaxed);

        if (!was_valid && c.counter.load(std::memory_order_relaxed) == 0) {
            return true;
        }
        throw;
    }
    return false;
}

template <typename... Vs>
bool WorkManager::all_done(Vs &&...vs)
{
    return (... && vs->done());
}
