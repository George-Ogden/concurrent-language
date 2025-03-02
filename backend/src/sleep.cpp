#include "main/user.hpp"
#include "system/thread_manager.tpp"
#include "time/utils.hpp"

#include <chrono>
#include <variant>

using namespace std::chrono_literals;

template <typename Rep, typename Period>
void sleep(const std::chrono::duration<Rep, Period> &duration) {
    auto start = time_utils::now();
    while (time_utils::now() - start < duration) {
    }
}

std::monostate main_sleep(std::monostate) {
    sleep(10s);
    return std::monostate{};
}

int main(int argc, char *argv[]) {
    typename ThreadManager::RunConfig config{
        ThreadManager::hardware_concurrency(), true};
    ThreadManager::run_multithreaded(main_sleep, std::monostate{}, config);
    return 0;
}
