#pragma once

#include <atomic>
#include <cstdint>
#include <tuple>
#include <type_traits>
#include <vector>

class Fn {
  protected:
    virtual void body() = 0;

  public:
    void run() {
        body();
        for (auto &cont : conts) {
            if (cont->deps.fetch_sub(1, std::memory_order_relaxed) == 1) {
                cont->run();
            }
        }
    }
    std::vector<Fn *> conts;
    std::atomic<uint32_t> deps;
};

template <typename Ret, typename... Args> struct ParametricFn : public Fn {
    using ArgsT = std::tuple<std::add_pointer_t<Args>...>;
    ArgsT args;
    Ret *ret;
};
