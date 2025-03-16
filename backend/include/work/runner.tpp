#pragma once

#include "data_structures/cyclic_queue.tpp"
#include "lazy/fns.hpp"
#include "system/work_manager.tpp"
#include "work/runner.hpp"

#include <atomic>
#include <deque>
#include <functional>
#include <memory>
#include <optional>

std::atomic<bool> WorkRunner::done_flag;
CyclicQueue<std::atomic<WorkT> *> WorkRunner::work_request_queue;

WorkRunner::WorkRunner(const ThreadManager::ThreadId &id) : id(id) {}

void WorkRunner::main(std::atomic<WorkT> *ref) {
    WorkT work = ref->exchange(nullptr, std::memory_order_relaxed);
    if (work != nullptr) {
        work->run();
        work->await_all();
        done_flag.store(true, std::memory_order_release);
    } else {
        while (!done_flag.load(std::memory_order_acquire)) {
            try {
                active_wait();
            } catch (finished &f) {
                break;
            }
        }
    }
}

template <typename T> constexpr auto filter_awaitable(T &v) {
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

template <typename... Vs> auto WorkRunner::await(Vs &...vs) {
    std::apply([&](auto &&...ts) { await_restricted(ts...); },
               std::tuple_cat(filter_awaitable(vs)...));
    return std::make_tuple(extract_lazy(vs)...);
}

template <typename T> void WorkRunner::await_variants(T &v) {}

template <typename... Ts>
void WorkRunner::await_variants(std::shared_ptr<Lazy<VariantT<Ts...>>> &l) {
    auto v = l->value();
    std::size_t idx = v.tag;

    std::function<void(std::aligned_union_t<0, Ts...> &)>
        waiters[sizeof...(Ts)] = {[this](auto &storage) {
            await_all(std::launder(reinterpret_cast<Ts *>(&storage))->value);
        }...};
    waiters[idx](v.value);
}

template <typename... Vs> void WorkRunner::await_all(Vs &...vs) {
    if constexpr (sizeof...(vs) != 0) {
        auto flat_types = flatten(std::make_tuple(vs...));
        std::apply([&](auto &&...ts) { await_restricted(ts...); }, flat_types);
        std::apply([&](auto &&...ts) { (await_variants(ts), ...); },
                   flat_types);
    }
}

template <typename... Vs> void WorkRunner::await_restricted(Vs &...vs) {
    constexpr unsigned n = sizeof...(vs);
    if constexpr (n == 0) {
        return;
    } else {
        if (all_done(vs...)) {
            return;
        }
        std::vector<WorkT> extra_works;
        std::vector<WorkT> small_works, large_works;
        do {
            while (large_works.size() > 1 && any_requests()) {
                if (respond(large_works.back())) {
                    large_works.pop_back();
                }
            }
            if (!small_works.empty()) {
                WorkT work = small_works.back();
                small_works.pop_back();
                work->run();
            } else if (!large_works.empty()) {
                WorkT work = large_works.back();
                large_works.pop_back();
                work->run();
            } else {
                (vs->get_work(extra_works), ...);
                if (extra_works.size() > 0) {
                    for (WorkT &work : extra_works) {
                        if (work->can_respond()) {
                            large_works.emplace_back(std::move(work));
                        } else {
                            small_works.emplace_back(std::move(work));
                        }
                    }
                    extra_works.clear();
                } else {
                    active_wait();
                }
            }

            if (all_done(vs...)) {
                return;
            }
        } while (!done_flag.load(std::memory_order_acquire));
        throw finished{};
    };
}

bool WorkRunner::any_requests() const { return !work_request_queue.empty(); }

void WorkRunner::active_wait() {
    WorkT work = request_work();
    work->run();
}

WorkT WorkRunner::request_work() const {
    std::atomic<WorkT> work{nullptr};
    work_request_queue.push(&work);
    while (work.load(std::memory_order_relaxed) == nullptr) {
        if (done_flag.load(std::memory_order_relaxed)) {
            throw finished{};
        }
    }
    return work.load(std::memory_order_relaxed);
}

bool WorkRunner::respond(WorkT &work) const {
    auto receiver = get_receiver();
    if (receiver.has_value()) {
        (*receiver)->store(work, std::memory_order_relaxed);
        return true;
    } else {
        return false;
    }
}

std::optional<std::atomic<WorkT> *> WorkRunner::get_receiver() const {
    return work_request_queue.pop();
}

template <typename... Vs> bool WorkRunner::all_done(Vs &&...vs) {
    return (... && vs->done());
}
