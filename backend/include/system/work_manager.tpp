#pragma once

#include "fn/fn.tpp"
#include "system/thread_manager.tpp"
#include "system/work_manager.hpp"
#include "data_structures/lazy.tpp"
#include "time/sleep.hpp"

#include <atomic>
#include <memory>
#include <utility>
#include <vector>
#include <chrono>

using namespace std::chrono_literals;

void FinishWork::run()
{
    throw finished{};
}
std::shared_ptr<Work> WorkManager::finish_work = std::make_shared<FinishWork>();

template <typename Ret, typename... Args>
Ret WorkManager::run(TypedFn<Ret, Args...> fn, Args...args)
{
    auto [work, result] = Work::fn_call(fn, args...);
    std::atomic<std::shared_ptr<Work>> ref{work};
    ThreadManager::RunConfig config{ThreadManager::available_concurrency(),
                                    false};
    WorkManager::queue->clear();
    WorkManager::counters = std::vector<std::atomic<unsigned>>(
        ThreadManager::available_concurrency());
    ThreadManager::run_multithreaded(main, &ref, config);
    WorkManager::queue->clear();
    return result;
}

void WorkManager::enqueue(std::shared_ptr<Work> work)
{
    WorkManager::queue.acquire();
    WorkManager::queue->push_back(work);
    WorkManager::queue.release();
}

std::monostate WorkManager::main(std::atomic<std::shared_ptr<Work>> *ref)
{
    {
        std::shared_ptr<Work> work =
            ref->exchange(nullptr, std::memory_order_relaxed);
        if (work != nullptr)
        {
            work->run();
            // work->await_all();
            enqueue(WorkManager::finish_work);
        }
        else
        {
            while (1)
            {
                work = get_work();
                if (work == nullptr)
                {
                    sleep(1us);
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

std::shared_ptr<Work> WorkManager::get_work()
{
    WorkManager::queue.acquire();
    if (WorkManager::queue->empty())
    {
        WorkManager::queue.release();
        return nullptr;
    }

    std::shared_ptr<Work> work = WorkManager::queue->front().lock();
    WorkManager::queue->pop_front();
    WorkManager::queue.release();
    return work;
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
void await_variants(T &v) {}

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
        delete valid;
        if (counter.fetch_sub(1, std::memory_order_relaxed) == 1)
        {
            return;
        }
        else
        {
            throw stack_inversion{};
        }
    }
    while (true)
    {
        std::shared_ptr<Work> work = get_work();
        if (dynamic_cast<FinishWork *>(work.get()) != nullptr)
        {
            enqueue(work);
            throw finished{};
        }
        try
        {
            if (counter.load(std::memory_order_relaxed) > 0)
            {
                throw stack_inversion{};
            }
            if (work != nullptr)
            {
                work->run();
            }
        }
        catch (stack_inversion &e)
        {
            valid->acquire();
            bool was_valid = **valid;
            **valid = false;
            valid->release();
            if (work != nullptr && !work->done())
            {
                enqueue(work);
            }
            if (!was_valid)
            {
                delete valid;
            }
            counter.fetch_sub(1 - was_valid, std::memory_order_relaxed);

            if (!was_valid && counter.load(std::memory_order_relaxed) == 0)
            {
                while (!all_done(vs...))
                {
                }
                return;
            }
            throw;
        }
    }
};

template <typename... Vs>
bool WorkManager::all_done(Vs &&...vs)
{
    return (... && vs->done());
}
