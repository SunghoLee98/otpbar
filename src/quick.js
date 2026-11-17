// 빠른 복사 창. 숫자 1~9로 복사하고 닫는다. 그 밖의 기능은 두지 않는다.

const invoke = window.__TAURI__.core.invoke;
const list = document.getElementById("list");

let items = [];
/// 복사 직후 닫히기 전까지 다른 입력을 받지 않는다.
let busy = false;

/// 이 창은 단축키로 불렀을 때만 잠깐 뜨므로 코드를 가리지 않는다.
function shown(item) {
  const code = item.code || "";
  if (!code) return "------";
  return code.length === 6 ? `${code.slice(0, 3)} ${code.slice(3)}` : code;
}

const SVG = "http://www.w3.org/2000/svg";
/// 원의 둘레. r=8 인 원에 맞춘다.
const RING = 2 * Math.PI * 8;
/// 이만큼 남으면 색을 바꿔 곧 바뀐다는 것을 알린다.
const SOON = 5;

function ring() {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", "ring");
  svg.setAttribute("viewBox", "0 0 20 20");
  for (const cls of ["track", "value"]) {
    const c = document.createElementNS(SVG, "circle");
    c.setAttribute("class", cls);
    c.setAttribute("cx", "10");
    c.setAttribute("cy", "10");
    c.setAttribute("r", "8");
    if (cls === "value") c.setAttribute("stroke-dasharray", String(RING));
    svg.appendChild(c);
  }
  return svg;
}

function setRing(li, item) {
  const left = item.remaining != null ? item.remaining : 0;
  const period = item.period || 30;
  const value = li.querySelector(".ring .value");
  // 새 주기로 넘어가 원이 다시 차오를 때는 거꾸로 감기는 움직임을 보이지 않는다
  const prev = Number(li.dataset.left || left);
  value.style.transition = left > prev ? "none" : "";
  li.dataset.left = String(left);
  value.setAttribute(
    "stroke-dashoffset",
    String(RING * (1 - left / period))
  );
  li.querySelector(".secs").textContent = String(left);
  li.classList.toggle("soon", left <= SOON);
}

function draw() {
  const same =
    list.children.length === items.length &&
    items.every((item, i) => list.children[i].dataset.id === item.id);

  if (!same) {
    list.replaceChildren(
      ...items.map((item, i) => {
        const li = document.createElement("li");
        li.dataset.id = item.id;

        const key = document.createElement("span");
        key.className = "key";
        key.textContent = String(i + 1);

        const label = document.createElement("span");
        label.className = "label";
        const issuer = document.createElement("span");
        issuer.className = "issuer";
        issuer.textContent = item.issuer || item.name;
        label.append(issuer);
        if (item.issuer && item.name) {
          const name = document.createElement("span");
          name.className = "name";
          name.textContent = item.name;
          label.append(name);
        }

        const code = document.createElement("span");
        code.className = "code";

        const timer = document.createElement("span");
        timer.className = "timer";
        const secs = document.createElement("span");
        secs.className = "secs";
        timer.append(ring(), secs);

        li.append(key, label, code, timer);
        li.addEventListener("click", () => pick(i));
        return li;
      })
    );
  }

  items.forEach((item, i) => {
    const li = list.children[i];
    li.querySelector(".code").textContent = shown(item);
    setRing(li, item);
  });
}

async function refresh() {
  try {
    const view = await invoke("quick_list");
    items = view.items;
    draw();
  } catch (_) {
    // 다음 주기에 다시 시도한다
  }
}

async function pick(index) {
  const item = items[index];
  if (!item || busy) return;
  busy = true;
  try {
    await invoke("quick_copy", { id: item.id });
    const li = list.children[index];
    li.classList.add("done");
    li.querySelector(".key").textContent = "✓";
    // 복사됐다는 것을 눈으로 확인할 만큼만 보여 주고 닫는다
    setTimeout(close, 180);
  } catch (_) {
    busy = false;
  }
}

function close() {
  invoke("quick_hide");
}

function reset() {
  busy = false;
  list.replaceChildren();
  refresh();
}

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    e.preventDefault();
    close();
    return;
  }
  if (e.metaKey || e.ctrlKey || e.altKey) return;
  if (/^[1-9]$/.test(e.key)) {
    e.preventDefault();
    pick(Number(e.key) - 1);
  }
});

// 숨겼다가 다시 띄울 때마다 새로 읽는다
window.addEventListener("focus", reset);

refresh();
setInterval(() => {
  if (document.hasFocus()) refresh();
}, 1000);
