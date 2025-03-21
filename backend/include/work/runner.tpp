#pragma once

#include "data_structures/cyclic_queue.tpp"
#include "data_structures/block_list.tpp"
#include "lazy/fns.hpp"
#include "system/work_manager.tpp"
#include "work/runner.hpp"
#include "work/work_request.tpp"

#include <atomic>
#include <functional>
#include <memory>
#include <optional>

std::atomic<bool> WorkRunner::done_flag{false};
CyclicQueue<unsigned> WorkRunner::work_request_queue;
std::vector<std::unique_ptr<WorkRequest>> WorkRunner::work_requests;

WorkRunner::WorkRunner(const ThreadManager::ThreadId &id) : id(id) {}

void WorkRunner::main(std::atomic<WorkT> *ref) {
    WorkT work = ref->exchange(nullptr, std::memory_order_relaxed);
    if (work != nullptr) {
        std::cout << static_cast<int>(ThreadManager::get_id()) << std::endl;
        work->run();
        work->await_all();
        done_flag.store(true, std::memory_order_relaxed);
    } else {
        while (!done_flag.load(std::memory_order_relaxed)) {
            try {
                active_wait(std::function<bool()>([](){return false;}));
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
                auto predicate = [&](){
                    if (all_done(vs...)){
                        return true;
                    }
                    (vs->get_work(extra_works), ...);
                    if (extra_works.size() > 0) {
                        return true;
                    } else {
                        return false;
                    }
                };
                if (predicate() || active_wait(std::function<bool()>(predicate))) {
                    for (WorkT &work : extra_works) {
                        if (work->can_respond()) {
                            large_works.emplace_back(std::move(work));
                        } else {
                            small_works.emplace_back(std::move(work));
                        }
                    }
                    extra_works.clear();
                }
            }

            if (all_done(vs...)) {
                return;
            }
        } while (!done_flag.load(std::memory_order_relaxed));
        throw finished{};
    };
}

bool WorkRunner::any_requests() const { return !work_request_queue.empty(); }

bool WorkRunner::active_wait(std::function<bool()> predicate) {
    WorkRequest &work_request = *work_requests[id];
    work_request.request();
    if (work_request.enqueue()) {
        work_request_queue.push(id);
    } else if (work_request.full()){
        work_request.fulfill();
        return false;
    }
    while (!predicate()) {
        if (work_request.full()){
            work_request.fulfill();
        }
        if (done_flag.load(std::memory_order_relaxed)) {
            throw finished{};
        }
    }
    if (!work_request.cancel()){
        assert(work_request.full());
        work_request.fulfill();
    }
    return true;
}

bool WorkRunner::respond(WorkT &work) const {
    auto idx = work_request_queue.pop();
    if (idx.has_value()) {
        WorkRequest &work_request = *work_requests[*idx];
        return work_request.fill(work);
    } else {
        return false;
    }
}

template <typename... Vs> bool WorkRunner::all_done(Vs &&...vs) {
    return (... && vs->done());
}

void WorkRunner::setup(unsigned num_cpus) {
    WorkRunner::num_cpus = num_cpus;
    WorkRunner::done_flag.store(false);
    WorkRunner::work_request_queue = CyclicQueue<unsigned>{num_cpus};
    WorkRunner::work_requests.clear();
    for (unsigned i = 0; i < num_cpus; i++){
        work_requests.emplace_back(std::move(std::make_unique<WorkRequest>()));
    }
    WorkRunner::work_requests.resize(num_cpus);
}
