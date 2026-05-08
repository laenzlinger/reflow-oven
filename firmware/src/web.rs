use anyhow::Result;
use esp_idf_svc::http::server::{Configuration, EspHttpServer};
use esp_idf_svc::http::Method;
use serde::Serialize;
use std::sync::{Arc, Mutex};

use crate::profile::Phase;

#[derive(Clone, Serialize)]
pub struct OvenState {
    pub temperature: f32,
    pub target: f32,
    pub duty_pct: f32,
    pub phase: Phase,
    pub simulating: bool,
    pub elapsed_s: f32,
}

impl Default for OvenState {
    fn default() -> Self {
        Self { temperature: 0.0, target: 0.0, duty_pct: 0.0, phase: Phase::Idle, simulating: false, elapsed_s: 0.0 }
    }
}

#[derive(Clone, Serialize)]
pub struct HistoryPoint {
    t: f32,
    temp: f32,
    target: f32,
}

pub struct History {
    pub points: Vec<HistoryPoint>,
}

impl History {
    pub fn new() -> Self {
        Self { points: Vec::with_capacity(600) }
    }

    pub fn push(&mut self, elapsed_s: f32, temp: f32, target: f32) {
        if self.points.len() >= 600 {
            self.points.remove(0);
        }
        self.points.push(HistoryPoint { t: elapsed_s, temp, target });
    }

    pub fn clear(&mut self) {
        self.points.clear();
    }
}

pub type SharedState = Arc<Mutex<OvenState>>;
pub type SharedHistory = Arc<Mutex<History>>;

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Reflow Oven</title>
<style>
body{font-family:monospace;max-width:700px;margin:0 auto;padding:1em;background:#111;color:#eee}
.val{font-size:2em;color:#0f0}.phase{color:#ff0}
button{padding:0.5em 1em;margin:0.3em;font-size:1em;cursor:pointer;border-radius:4px;border:1px solid #555;background:#222;color:#eee}
button:hover{background:#333}
canvas{width:100%;height:250px;background:#1a1a1a;border:1px solid #333;border-radius:4px}
.controls{margin:1em 0}
</style></head><body>
<h1>Reflow Oven</h1>
<p>Temperature: <span class="val" id="temp">--</span> &deg;C</p>
<p>Target: <span id="target">--</span> &deg;C | Duty: <span id="duty">--</span>% | Phase: <span class="phase" id="phase">Idle</span></p>
<canvas id="chart" height="250"></canvas>
<div class="controls">
<select id="profile" onchange="fetch('/profile',{method:'POST',body:this.value})">
<option value="sn63pb37">Sn63/Pb37 (183°C)</option>
<option value="sn42bi58">Sn42/Bi58 (138°C)</option>
</select>
<button onclick="fetch('/start',{method:'POST'})">Start</button>
<button onclick="fetch('/stop',{method:'POST'})">Stop</button>
<button onclick="fetch('/simulate',{method:'POST'})" id="simbtn">Simulate: OFF</button>
</div>
<p style="font-size:0.8em;color:#888">LED: <span style="color:#a0f">&#x25cf;</span> connecting <span style="color:#00f">&#x25cf;</span> idle <span style="color:#f80">&#x25cf;</span> preheat <span style="color:#ff0">&#x25cf;</span> soak <span style="color:#f00">&#x25cf;</span> reflow <span style="color:#0cf">&#x25cf;</span> cooling <span style="color:#0f0">&#x25cf;</span> done</p>
<script>
const canvas=document.getElementById('chart'),ctx=canvas.getContext('2d');
function resize(){canvas.width=canvas.clientWidth;canvas.height=canvas.clientHeight}
resize();window.onresize=resize;

function drawChart(hist){
  const W=canvas.width,H=canvas.height,pad=30;
  ctx.clearRect(0,0,W,H);
  if(!hist.length)return;
  const maxT=Math.max(220,...hist.map(p=>Math.max(p.temp,p.target)));
  const maxTime=Math.max(60,hist[hist.length-1].t);
  const x=t=>(t/maxTime)*(W-pad*2)+pad;
  const y=v=>H-pad-(v/maxT)*(H-pad*2);

  // Grid
  ctx.strokeStyle='#333';ctx.lineWidth=0.5;ctx.beginPath();
  for(let v=0;v<=maxT;v+=50){ctx.moveTo(pad,y(v));ctx.lineTo(W-pad,y(v));}
  ctx.stroke();
  ctx.fillStyle='#666';ctx.font='10px monospace';
  for(let v=0;v<=maxT;v+=50)ctx.fillText(v+'°',2,y(v)+3);

  // Target line
  ctx.strokeStyle='#555';ctx.lineWidth=2;ctx.setLineDash([4,4]);ctx.beginPath();
  hist.forEach((p,i)=>{i?ctx.lineTo(x(p.t),y(p.target)):ctx.moveTo(x(p.t),y(p.target));});
  ctx.stroke();ctx.setLineDash([]);

  // Temperature line
  ctx.strokeStyle='#0f0';ctx.lineWidth=2;ctx.beginPath();
  hist.forEach((p,i)=>{i?ctx.lineTo(x(p.t),y(p.temp)):ctx.moveTo(x(p.t),y(p.temp));});
  ctx.stroke();
}

function poll(){
  fetch('/status').then(r=>r.json()).then(d=>{
    document.getElementById('temp').textContent=d.temperature.toFixed(1);
    document.getElementById('target').textContent=d.target.toFixed(0);
    document.getElementById('duty').textContent=d.duty_pct.toFixed(0);
    document.getElementById('phase').textContent=d.phase;
    document.getElementById('simbtn').textContent='Simulate: '+(d.simulating?'ON':'OFF');
  });
  fetch('/history').then(r=>r.json()).then(drawChart);
  setTimeout(poll,1000);
}
poll();
</script></body></html>"#;

pub fn start_server(state: SharedState, history: SharedHistory) -> Result<EspHttpServer<'static>> {
    let mut server = EspHttpServer::new(&Configuration::default())?;

    server.fn_handler("/", Method::Get, move |req| {
        let len = INDEX_HTML.len().to_string();
        let headers = [("Content-Type", "text/html"), ("Content-Length", len.as_str())];
        let mut resp = req.into_response(200, Some("OK"), &headers)?;
        resp.write(INDEX_HTML.as_bytes())?;
        Ok::<(), esp_idf_svc::io::EspIOError>(())
    })?;

    let st = state.clone();
    server.fn_handler("/status", Method::Get, move |req| {
        let s = st.lock().unwrap().clone();
        let json = serde_json::to_string(&s).unwrap();
        let len = json.len().to_string();
        let headers = [("Content-Type", "application/json"), ("Content-Length", len.as_str())];
        let mut resp = req.into_response(200, Some("OK"), &headers)?;
        resp.write(json.as_bytes())?;
        Ok::<(), esp_idf_svc::io::EspIOError>(())
    })?;

    let hist = history.clone();
    server.fn_handler("/history", Method::Get, move |req| {
        let h = hist.lock().unwrap();
        let json = serde_json::to_string(&h.points).unwrap();
        let len = json.len().to_string();
        let headers = [("Content-Type", "application/json"), ("Content-Length", len.as_str())];
        let mut resp = req.into_response(200, Some("OK"), &headers)?;
        resp.write(json.as_bytes())?;
        Ok::<(), esp_idf_svc::io::EspIOError>(())
    })?;

    Ok(server)
}
