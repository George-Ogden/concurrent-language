#pragma once

#include "work/runner.hpp"
#include "system/work_manager.tpp"

#include <atomic>
#include <functional>
#include <chrono>
#include <memory>
#include <deque>

using namespace std::chrono_literals;

WorkRunner::WorkRunner(const ThreadManager::ThreadId &id):id(id){}

void WorkRunner::main(std::atomic<WorkT> *ref){
    WorkT work = ref->exchange(nullptr, std::memory_order_relaxed);
    if (work != nullptr) {
        priority_mode = true;
        work->status.require();
        work->run();
        priority_mode = true;
        work->await_all();
        done_flag.store(true, std::memory_order_release);
    } else {
        while (!done_flag.load(std::memory_order_acquire)) {
            std::tie(work, priority_mode) = get_work();
            if (work == nullptr) {
                continue;
            }
            try {
                work->run();
            } catch (finished &e) {
                break;
            }
        }
    }

}

void WorkRunner::enqueue(WorkT work) {
    if (work->status.enqueue()){
        WorkRunner::shared_work_queue.acquire();
        WorkRunner::shared_work_queue->push_back(work);
        WorkRunner::shared_work_queue.release();
    }
}

void WorkRunner::try_priority_enqueue(WorkT work) {
    if (work->status.require()){
        priority_enqueue(work);
    }
}

void WorkRunner::priority_enqueue(WorkT work) {
    private_work_stack.acquire();
    private_work_stack->push_back(work);
    private_work_stack.release();
}

std::pair<WorkT,bool> WorkRunner::get_work() {
    for (std::size_t offset = 0; offset < (num_cpus << 1); offset ++){
        std::size_t gray_code = (offset>>1)^offset;
        ThreadManager::ThreadId idx = offset ^ gray_code;
        if (idx < num_cpus){
            Locked<std::deque<WorkT>> &stack = offset == 0 ? private_work_stack : WorkManager::runners[idx]->private_work_stack;
            if (offset == 0){
                stack.acquire();
                if (stack->empty()) {
                    stack.release();
                } else {
                    WorkT work = stack->back();
                    stack->pop_back();
                    stack.release();
                    return std::make_pair(work, true);
                }
            } else if (!stack->empty() && stack.try_acquire()){
                if (stack->empty()) {
                    stack.release();
                } else {
                    WorkT work = stack->back();
                    stack->pop_back();
                    stack.release();
                    return std::make_pair(work, true);
                }
            }
        }
    }

    Locked<std::deque<WeakWorkT>> &queue = shared_work_queue;
    if (!queue->empty()){
        queue.acquire();
        while (!queue->empty()) {
            WorkT work = queue->front().lock();
            queue->pop_front();
            if (work != nullptr && work->status.dequeue()){
                queue.release();
                return std::make_pair(work, false);
            }
        }
        queue.release();
    }
    return std::make_pair(nullptr, false);
}

template <typename T>
constexpr auto filter_awaitable(T &v) {
    return std::tuple<std::decay_t<T>>(v);
}

template <typename... Ts>
constexpr auto filter_awaitable(std::tuple<Ts...> &v) {
    return std::tuple<>{};
}

template <typename... Vs>
void WorkRunner::await(Vs &...vs) {
    std::apply([&](auto &&...ts)
               { await_restricted(ts...); },
               std::tuple_cat(filter_awaitable(vs)...));
}

template <typename T>
void WorkRunner::await_variants(T &v) { }

template <typename... Ts>
void WorkRunner::await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l) {
    auto v = l->value();
    std::size_t idx = v.tag;

    std::function<void(std::aligned_union_t<0, Ts...> &)> waiters[sizeof...(Ts)] = {[this](auto &storage){ await_all( std::launder(reinterpret_cast<Ts *>(&storage))->value); }...};
    waiters[idx](v.value);
}

template <typename... Vs>
void WorkRunner::await_all(Vs &...vs) {
    if constexpr (sizeof...(vs) != 0) {
        auto flat_types = flatten(std::make_tuple(vs...));
        std::apply([&](auto &&...ts)
                   { await_restricted(ts...); }, flat_types);
        std::apply([&](auto &&...ts)
                   { (await_variants(ts), ...); },
                   flat_types);
    }
}

template <typename... Vs>
void WorkRunner::await_restricted(Vs &...vs) {
    unsigned n = sizeof...(vs);
    if (n == 0) {
        return;
    }
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{n};
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    (vs->add_continuation(c), ...);
    if (all_done(vs...)) {
        return exit_early(c);
    }
    if (priority_mode){
        (vs->prioritize(), ...);
    }
    while (!done_flag.load(std::memory_order_acquire))
    {
        auto [work, work_priority] = get_work();
        if (break_on_work(std::make_pair(work, work_priority), c)){
            if (done_flag.load(std::memory_order_acquire)){
                break;
            }
            return;
        }
    }
    throw finished{};
};

void WorkRunner::exit_early(Continuation &c){
    delete c.valid;
    if (c.counter.fetch_sub(1, std::memory_order_relaxed) == 1) {
        return;
    } else {
        throw stack_inversion{};
    }
}

bool WorkRunner::break_on_work(std::pair<WorkT,bool> work_pair, Continuation &c){
    const bool prev_priority = priority_mode;
    auto &[work, work_priority] = work_pair;
    try {
        if (c.counter.load(std::memory_order_relaxed) > 0) {
            throw stack_inversion{};
        }
        if (work != nullptr) {
            priority_mode = work_priority;
            if (!work->run() && work_priority){
                priority_enqueue(work);
            }
            priority_mode = prev_priority;
        }
    } catch (stack_inversion &e) {
        c.valid->acquire();
        bool was_valid = **c.valid;
        **c.valid = false;
        c.valid->release();
        if (work != nullptr && !work->done()){
            work->status.cancel_work();
            if (work_priority){
                priority_enqueue(work);
            } else {
                enqueue(work);
            }
        }
        if (!was_valid) {
            delete c.valid;
        }
        c.counter.fetch_sub(1 - was_valid, std::memory_order_relaxed);

        priority_mode = prev_priority;
        if (!was_valid && c.counter.load(std::memory_order_relaxed) == 0) {
            return true;
        }
        throw;
    }
    return false;
}

template <typename... Vs>
bool WorkRunner::all_done(Vs &&...vs) {
    return (... && vs->done());
}
