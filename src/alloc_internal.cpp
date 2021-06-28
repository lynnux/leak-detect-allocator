#include <mutex>
#include <map>
#include <vector>
#include <atomic>
#include <algorithm>

struct AllocInternal
{
    std::map<size_t, std::vector<size_t>> gLeakData;
    std::mutex gMtx;
    size_t gStackSize = 10;
    size_t gOrder = 0;
};
AllocInternal *gAllocInternal = new AllocInternal(); // let it leak, because rust will call dealloc after cpp.exit(msvc)

extern "C"
{
    void alloc_internal_init(size_t stack_size)
    {
        gAllocInternal->gStackSize = stack_size + 2; // 0 for size, 1 for order
    }

    // stack point to size_t[10]
    void alloc_internal_alloc(size_t address, size_t size, size_t *stack)
    {
        gAllocInternal->gMtx.lock();
        std::vector<size_t> vs;
        vs.push_back(size);
        vs.push_back(gAllocInternal->gOrder);
        for (size_t i = 0; i < gAllocInternal->gStackSize - 2; ++i)
            vs.push_back(stack[i]);
        gAllocInternal->gLeakData.insert(std::make_pair(address, std::move(vs)));
        ++gAllocInternal->gOrder;
        gAllocInternal->gMtx.unlock();
    }

    void alloc_internal_dealloc(size_t address)
    {
        gAllocInternal->gMtx.lock();
        auto fi = gAllocInternal->gLeakData.find(address);
        if (fi != gAllocInternal->gLeakData.end())
            gAllocInternal->gLeakData.erase(fi);
        gAllocInternal->gMtx.unlock();
    }

    void alloc_enum(void *usr_data, int(__cdecl *cb)(void *usr_data, size_t address, size_t size, const size_t *stack))
    {
        std::vector<std::vector<size_t>> all;
        gAllocInternal->gMtx.lock();
        for (const auto &i : gAllocInternal->gLeakData)
        {
            std::vector<size_t> v;
            v.reserve(gAllocInternal->gStackSize + 1);
            v.push_back(i.first); // 0 address
            for (const auto &j : i.second)
            {
                v.push_back(j); // 1 size, 2 order
            }
            all.push_back(std::move(v));
        }
        gAllocInternal->gMtx.unlock();
        std::sort(all.begin(), all.end(), [](const std::vector<size_t> &left, const std::vector<size_t> &right)
                  { return left[2] < right[2]; });
        for (const auto &i : all)
        {
            if (0 == cb(usr_data, i[0], i[1], &i[3]))
                break;
        }
    }
}