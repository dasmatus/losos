---
tags: soil
---

# Compost

## Hot compost

Mix greens and browns at roughly $C:N \approx 30:1$ by weight, keep it as
damp as a wrung sponge and turn it every week. ^ratio

> [!NOTE]
> A heap needs about a cubic metre to heat up properly.

$$
V = l \times w \times h \geq 1\,\text{m}^3
$$

## Cold compost

Pile it up and wait a year. Good for leaves.

```bash
# Log the heap's temperature every morning
echo "$(date -I) $(cat /sys/bus/w1/devices/28-*/temperature)" >> heap.log
```

Used on [[Tomatoes]] and in the [[Planting calendar]].
