import sys
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
plt.rcParams["font.family"] = "IPAGothic"
out = sys.argv[1]
r = list(range(1, 8))
ws = [6.541, 10.291, 9.396, 9.769, 10.279, 10.694, 10.714]
ci = [[1.261,1.560,1.585,1.611,1.732,1.784,1.891],
      [1.283,1.559,1.609,1.653,1.711,1.750,1.791],
      [1.245,1.541,1.594,1.616,1.676,1.736,1.798]]
ci_mean = [sum(x)/3 for x in zip(*ci)]
fig, ax = plt.subplots(figsize=(8, 4.5), dpi=150)
fig.patch.set_facecolor("#fcfcfb"); ax.set_facecolor("#fcfcfb")
ax.plot(r, ws, color="#2a78d6", lw=2, marker="o", ms=7, label="ワークステーション（QEMU VM、1 回実行）")
ax.plot(r, ci_mean, color="#eb6834", lw=2, marker="o", ms=7, label="CI（ubuntu-24.04、3 回の平均）")
ax.axhline(10, color="#52514e", lw=1.2, ls="--")
ax.text(7.25, 10.25, "旧上限\n10 秒", va="bottom", color="#52514e", fontsize=9)
ax.text(1, 11.6, "新上限は 60 秒（図の範囲外）", color="#52514e", fontsize=9)
ax.set_xlim(0.7, 7.9); ax.set_ylim(0, 12.5)
ax.set_xlabel("交換の回数"); ax.set_ylabel("交換 1 回の所要時間（秒）")
ax.set_title("大きなスナップショットの gossip 交換 1 回あたりの所要時間（debug ビルド）", fontsize=11)
ax.grid(axis="y", color="#e6e5e0", lw=0.8); ax.set_axisbelow(True)
for s in ("top", "right"): ax.spines[s].set_visible(False)
for s in ("left", "bottom"): ax.spines[s].set_color("#b5b4ad")
ax.legend(loc="center right", frameon=False, fontsize=9)
fig.tight_layout(); fig.savefig(out)
