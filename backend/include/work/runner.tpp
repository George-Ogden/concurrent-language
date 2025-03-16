#pragma once

#include "lazy/fns.hpp"
#include "work/runner.hpp"
#include "system/work_manager.tpp"
#include "data_structures/cyclic_queue.tpp"
#include "work/comparator.hpp"

#include <atomic>
#include <functional>
#include <chrono>
#include <memory>
#include <deque>
#include <optional>

using namespace std::chrono_literals;

std::atomic<bool> WorkRunner::done_flag;
CyclicQueue<std::atomic<WorkT> *> WorkRunner::work_request_queue;

WorkRunner::WorkRunner(const ThreadManager::ThreadId &id):id(id){}

void WorkRunner::main(std::atomic<WorkT> *ref){
    WorkT work = ref->exchange(nullptr, std::memory_order_relaxed);
    if (work != nullptr) {
        work->run();
        work->await_all();
        done_flag.store(true, std::memory_order_release);
    } else {
        while (!done_flag.load(std::memory_order_acquire)) {
            try {
                active_wait();
            } catch (finished &f){
                break;
            }
        }
    }
}

template <typename T>
constexpr auto filter_awaitable(T &v) {
    if constexpr (is_lazy_v<T>) {
        return std::tuple<std::decay_t<T>>(v);
    } else {
        return std::tuple<>{};
    }
}

template <typename... Ts>
constexpr auto filter_awaitable(std::tuple<Ts...> &v) {
    return std::tuple<>{};
}

template <typename... Vs>
auto WorkRunner::await(Vs &...vs) {
    std::apply([&](auto &&...ts)
               { await_restricted(ts...); },
               std::tuple_cat(filter_awaitable(vs)...));
    return std::make_tuple(extract_lazy(vs)...);
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
    if (all_done(vs...)){
        return;
    }
    std::atomic<unsigned> *remaining = new std::atomic<unsigned>{n};
    Locked<bool> *valid = new Locked<bool>{true};
    Continuation c{remaining, counter, valid};
    (vs->add_continuation(c), ...);
    std::vector<WorkT> works;
    (vs->get_work(works), ...);
    std::size_t i = 0;
    bool sorted = false;
    while (!done_flag.load(std::memory_order_acquire))
    {
        while (i < works.size() - 1 && any_requests()){
            if (!sorted){
                std::sort(works.begin() + i, works.end(), WorkSizeComparator());
                sorted = true;
            }
            if (works.back()->can_respond()){
                if (respond(works.back())) {
                    works.pop_back();
                }
            }
        }
        if (i >= works.size()){
            active_wait();
            continue;
        }

        WorkT &work = works[i];
        if (break_on_work(work, c)){
            if (done_flag.load(std::memory_order_acquire)){
                break;
            }
            while (!all_done(vs...)) {}
            return;
        }
        i++;
    }
    throw finished{};
};

bool WorkRunner::any_requests() const {
    return !work_request_queue.empty();
}

void WorkRunner::active_wait(){
    WorkT work = request_work();
    work->run();
}

WorkT WorkRunner::request_work() const {
    std::atomic<WorkT> work{nullptr};
    work_request_queue.push(&work);
    while (work.load(std::memory_order_relaxed) == nullptr) {
        if (done_flag.load(std::memory_order_relaxed)){
            throw finished{};
        }
    }
    return work.load(std::memory_order_relaxed);
}

bool WorkRunner::respond(WorkT &work) const {
    auto receiver = get_receiver();
    if (receiver.has_value()){
        (*receiver)->store(work, std::memory_order_relaxed);
        return true;
    } else {
        return false;
    }
}

std::optional<std::atomic<WorkT>*> WorkRunner::get_receiver() const {
    return work_request_queue.pop();
}

bool WorkRunner::break_on_work(WorkT &work, Continuation &c){
    try {
        if (c.counter.load(std::memory_order_relaxed) > 0) {
            throw stack_inversion{};
        }
        if (work != nullptr) {
            work->run();
        }
        return false;
    } catch (stack_inversion &e) {
        c.valid->acquire();
        bool was_valid = **c.valid;
        **c.valid = false;
        c.valid->release();
        if (work != nullptr && !work->done()){
            // process(work);
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
}

template <typename... Vs>
bool WorkRunner::all_done(Vs &&...vs) {
    return (... && vs->done());
}
