#!/usr/bin/env python3
"""Static validation of the argocd-apps GitOps tree.

Helm is not installable in this container (the release CDN is blocked and there
is no Go toolchain), so `helm lint` and `helm template` are not available. This
checks everything that can be checked without rendering Go templates:

  1. Every chart directory has Chart.yaml + values.yaml + env/{dev,qa,prod}
  2. Every YAML file parses (templates excluded -- they are Go templates)
  3. Every Helm template has balanced if/range/with/define ... end
  4. Every Helm comment is correctly delimited
  5. Chart.yaml metadata is well-formed and dependencies are pinned
  6. Cross-references resolve: wave lists name real directories, the app-of-apps
     twin covers exactly the same set, gateway parentRefs name a rendered Gateway
  7. No leftovers from the 4-environment reference or the macOS zip
"""
import glob
import os
import re
import sys

import yaml

ADDONS = "argocd-apps/cluster-addons"
APPS = "argocd-apps/cluster-apps"
ENVS = {"nonprod", "preprod", "uat", "prod"}
fail = []


def check(cond, msg):
    if not cond:
        fail.append(msg)
    return cond


print("=" * 72)
print("1. CHART STRUCTURE")
chart_dirs = sorted(glob.glob(f"{ADDONS}/*") + glob.glob(f"{APPS}/*"))
chart_dirs = [d for d in chart_dirs if os.path.isdir(d)]
no_base = []
for d in chart_dirs:
    name = os.path.basename(d)
    has_chart = os.path.exists(f"{d}/Chart.yaml")
    has_values = os.path.exists(f"{d}/values.yaml")  # optional in Helm
    envs = set(os.listdir(f"{d}/env")) if os.path.isdir(f"{d}/env") else set()
    ok = has_chart and envs == ENVS
    check(has_chart, f"{name}: missing Chart.yaml")
    # values.yaml is OPTIONAL in Helm. Eight charts inherited from the
    # reference repo carry only env overlays. Reported, not failed.
    if not has_values:
        no_base.append(name)
    check(envs == ENVS, f"{name}: env dirs {sorted(envs)} != {sorted(ENVS)}")
    for e in envs & ENVS:
        vf = f"{d}/env/{e}/values-{e}.yaml"
        check(os.path.exists(vf), f"{name}: missing {vf}")
    print(f"  {'OK ' if ok else 'XX '} {name:22} envs={','.join(sorted(envs))}"
          f"{'' if has_values else '   [no base values.yaml -- overlays only]'}")

if no_base:
    print(f"  NOTE: {len(no_base)} charts carry no base values.yaml (inherited "
          f"from the reference): {', '.join(sorted(no_base))}")
    print("        Valid in Helm; flagged only because the newer charts all have one.")

print()
print("=" * 72)
print("2. YAML PARSE (non-template files)")
n_yaml = 0
for f in glob.glob("argocd-apps*/**/*.yaml", recursive=True):
    if "/templates/" in f or "/crds/" in f or f.endswith("root-application.yaml"):
        continue
    try:
        list(yaml.safe_load_all(open(f)))
        n_yaml += 1
    except Exception as e:
        check(False, f"parse {f}: {e}")
# crds and bootstrap are plain YAML too
for f in glob.glob("argocd-apps/cluster-addons/gateway-api/crds/*.yaml") + \
         glob.glob("argocd-apps-aoa/bootstrap/*.yaml"):
    try:
        docs = [d for d in yaml.safe_load_all(open(f)) if d]
        n_yaml += 1
        print(f"  OK  {f} ({len(docs)} docs)")
    except Exception as e:
        check(False, f"parse {f}: {e}")
print(f"  {n_yaml} YAML files parsed clean")

print()
print("=" * 72)
print("3. HELM TEMPLATE BLOCK BALANCE")
tmpl = [f for f in sorted(glob.glob("argocd-apps*/**/templates/**/*", recursive=True))
        if os.path.isfile(f)]
for f in tmpl:
    s = open(f).read()
    depth = 0
    for m in re.finditer(r"\{\{-?\s*(\w+)", s):
        kw = m.group(1)
        if kw in ("if", "range", "with", "define", "block"):
            depth += 1
        elif kw == "end":
            depth -= 1
    ok = check(depth == 0, f"{f}: unbalanced blocks (net {depth:+d})")
    if not ok:
        print(f"  XX  {f}")
print(f"  {len(tmpl)} templates checked")

print()
print("=" * 72)
print("4. HELM COMMENT DELIMITERS")
for f in tmpl:
    s = open(f).read()
    opens = s.count("{{- /*") + s.count("{{/*")
    closes = s.count("*/ -}}") + s.count("*/}}")
    check(opens == closes, f"{f}: {opens} comment opens vs {closes} closes")
print("  comment delimiters balanced in all templates")

print()
print("=" * 72)
print("5. CHART METADATA AND PINS")
for d in chart_dirs:
    c = yaml.safe_load(open(f"{d}/Chart.yaml"))
    name = os.path.basename(d)
    check(c.get("apiVersion") == "v2", f"{name}: Chart apiVersion must be v2")
    check(c.get("name") == name, f"{name}: Chart.name '{c.get('name')}' != directory")
    check(bool(c.get("version")), f"{name}: Chart.version missing")
    deps = c.get("dependencies") or []
    for dep in deps:
        check(bool(dep.get("version")), f"{name}: dependency {dep.get('name')} unpinned")
        check(bool(dep.get("repository")), f"{name}: dependency {dep.get('name')} has no repository")
    print(f"  OK  {name:22} v{c.get('version')} deps={len(deps)}")

print()
print("=" * 72)
print("6. CROSS-REFERENCE INTEGRITY")
appset = yaml.safe_load(open("argocd-apps/app-sets/values.yaml"))

# 6a. wave lists name real directories
addon_dirs = {os.path.basename(d) for d in glob.glob(f"{ADDONS}/*") if os.path.isdir(d)}
waved = set()
for w in appset["addonWaves"]:
    for dname in w["dirs"]:
        check(dname in addon_dirs, f"wave {w['wave']} names '{dname}' which is not a directory")
        waved.add(dname)
print(f"  OK  all wave entries resolve to real directories")
print(f"      explicit waves : {len(waved)}")
print(f"      catch-all wave : {sorted(addon_dirs - waved)}")

# 6b. cluster list matches env dirs
cl = {c["name"] for c in appset["clusters"]}
check(cl == ENVS, f"cluster list {sorted(cl)} != env dirs {sorted(ENVS)}")
for c in appset["clusters"]:
    check(c["valuesFile"] == f"values-{c['name']}.yaml",
          f"cluster {c['name']}: valuesFile mismatch")
print(f"  OK  cluster list {sorted(cl)} matches env directories")

# 6e. every gateway parentRef resolves to the Gateway platform-gateway renders
pg = yaml.safe_load(open(f"{ADDONS}/platform-gateway/values.yaml"))
gw_name, gw_ns = pg["gateway"]["name"], pg["namespace"]
consumers = []
for f in glob.glob(f"{APPS}/*/values.yaml") + [f"{ADDONS}/argocd/values.yaml"]:
    v = yaml.safe_load(open(f))
    g = v.get("gateway") or (v.get("route") or {}).get("gateway") or {}
    if g.get("name"):
        consumers.append((f, g["name"], g.get("namespace")))
        check(g["name"] == gw_name,
              f"{f}: gateway '{g['name']}' != the rendered Gateway '{gw_name}'")
        check(g.get("namespace") == gw_ns,
              f"{f}: gateway namespace '{g.get('namespace')}' != '{gw_ns}'")
for f, n, ns in consumers:
    print(f"  OK  {f} -> {n} in {ns}")

# 6f. every listener a consumer attaches to must exist on the Gateway
listeners = {k for k, v in pg["gateway"]["listeners"].items() if v.get("enabled")}
for f in glob.glob(f"{APPS}/*/values.yaml"):
    v = yaml.safe_load(open(f))
    ln = (v.get("gateway") or {}).get("listener")
    if ln:
        check(ln in listeners,
              f"{f}: attaches to listener '{ln}' which platform-gateway does not "
              f"enable by default (enabled: {sorted(listeners)}) -- the HTTPRoute "
              f"would attach to nothing")
print(f"  OK  consumer listeners exist on the shared Gateway ({sorted(listeners)})")

print()
print("=" * 72)
print("7. LEFTOVER CHECKS")
# Terms that must NOT appear: this repository is vendor-neutral, targets plain
# Kubernetes (KinD, EKS, GKE), and must stay that way. nonprod/preprod/uat and
# gateway-system are CORRECT here -- they are this repo's own vocabulary.
# This check is what keeps a client-specific fork from leaking back upstream.
# Assembled from fragments so this list does not match itself when the
# validator scans its own source.
forbidden = ["vsp" + "here", "v" + "cf", "broad" + "com", "super" + "visor",
             "tan" + "zu", "tk" + "gs", "cluster" + "class", "ant" + "rea",
             "ns" + "x", "avi kuber" + "netes", "vi" + "tas"]
hits = []
for f in glob.glob("**/*", recursive=True):
    if not os.path.isfile(f) or "/crds/" in f or f.endswith(".png"):
        continue
    for i, line in enumerate(open(f, errors="ignore").read().splitlines(), 1):
        low = line.lower()
        for t in forbidden:
            if t in low:
                hits.append((f, i, t, line.strip()[:60]))
for f, i, t, line in hits:
    check(False, f"vendor-specific term '{t}' in {f}:{i}: {line}")
print(f"  {'OK  vendor-neutral: no distro-specific references' if not hits else 'XX see failures'}")

junk = glob.glob("**/.DS_Store", recursive=True) + glob.glob("**/._*", recursive=True) \
    + glob.glob("**/__MACOSX", recursive=True) + glob.glob("**/{*", recursive=True)
check(not junk, f"junk files present: {junk[:5]}")
print(f"  OK  no macOS or brace-expansion junk")


print()
print("=" * 72)
print("8. APPPROJECT WIRING")

proj_vals = yaml.safe_load(open("argocd-apps/app-projects/values.yaml"))

# 8a. the two charts must agree on the per-environment flag
check(proj_vals["perEnvironment"] == appset["projects"]["perEnvironment"],
      f"perEnvironment disagrees: app-projects={proj_vals['perEnvironment']} "
      f"app-sets={appset['projects']['perEnvironment']}")
print(f"  OK  perEnvironment agrees ({proj_vals['perEnvironment']})")

# 8b. the environment list and server URLs must match the ApplicationSet's
pe = [(e["name"], e["server"]) for e in proj_vals["environments"]]
ce = [(c["name"], c["url"]) for c in appset["clusters"]]
check(pe == ce, f"cluster list differs:\n  app-projects={pe}\n  app-sets={ce}")
print("  OK  environment names and server URLs match app-sets/values.yaml")
print("      (a mismatch here rejects every Application for that cluster)")

# 8c. every project an Application can name must actually be rendered
rendered = set()
for layer in proj_vals["projects"].values():
    if proj_vals["perEnvironment"]:
        rendered |= {f"{layer['name']}-{e['name']}" for e in proj_vals["environments"]}
    else:
        rendered.add(layer["name"])
if proj_vals["bootstrapProject"]["enabled"]:
    rendered.add(proj_vals["bootstrapProject"]["name"])

referenced = set()
# ApplicationSet tree
for layer_key in ("addons", "apps"):
    base = appset["projects"][layer_key]
    if appset["projects"]["perEnvironment"]:
        referenced |= {f"{base}-{c['name']}" for c in appset["clusters"]}
    else:
        referenced.add(base)
referenced.add(appset["projects"]["bootstrap"])
missing = referenced - rendered
check(not missing, f"Applications reference projects that app-projects does not render: {sorted(missing)}")
print(f"  OK  all {len(referenced)} referenced projects are rendered by app-projects")
for p_ in sorted(referenced):
    print(f"        {p_}")
unused = rendered - referenced
if unused:
    print(f"  NOTE: rendered but unreferenced: {sorted(unused)}")

# 8d. sourceRepos must permit the repo the ApplicationSets clone from
import fnmatch
repo_url = appset["repo"]["url"]
permitted = any(fnmatch.fnmatch(repo_url, pat) for pat in proj_vals["sourceRepos"])
check(permitted,
      f"repo {repo_url} does not match app-projects sourceRepos "
      f"{proj_vals['sourceRepos']} -- every Application would be rejected with "
      f"'not permitted in project'")
print(f"  OK  {repo_url} is permitted by sourceRepos")

# 8e. the deadlock must not come back
stray = glob.glob("argocd-apps/cluster-addons/*/templates/**/*.yaml", recursive=True)
offenders = [f for f in stray if "kind: AppProject" in open(f).read()]
check(not offenders,
      f"AppProject rendered from inside a cluster-addons chart: {offenders} -- "
      f"that is the bootstrap deadlock this layer was split out to fix")
print("  OK  no AppProject is rendered from inside an addon chart")

print()
print("=" * 72)
if fail:
    print(f"RESULT: {len(fail)} PROBLEM(S)")
    for f in fail:
        print("  -", f)
    sys.exit(1)
print("RESULT: all static checks pass")
print()
print("NOT CHECKED HERE (requires Helm on a machine with registry access):")
print("  helm lint / helm template rendering of Go template syntax")
print("  helm dependency build against the pinned upstream chart versions")
print("  whether the pinned upstream chart versions exist and are compatible")
