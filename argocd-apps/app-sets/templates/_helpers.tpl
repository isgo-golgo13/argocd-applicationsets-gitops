{{/*
---------------------------------------------------------------------------
ArgoCD-side placeholders.

app-sets/ is a Helm chart whose templates ARE ApplicationSet manifests, so two
templating engines both claim {{ }}. Helm renders first; anything it consumes
never reaches the ApplicationSet controller.

Every placeholder that must survive Helm and be resolved later by ArgoCD is
defined here inside backticks. Go treats a backticked string as a literal, so
{{ `{{.path.basename}}` }} emits {{.path.basename}} untouched.

Keeping all of them in this file means the escaping is auditable in one place
instead of scattered across two ApplicationSet manifests.

All ApplicationSets in this chart set goTemplate: true, so placeholders use
dotted Go template form ({{.cluster}}), NOT the deprecated fasttemplate form
({{cluster}}).
---------------------------------------------------------------------------
*/}}

{{/* Cluster name, from the list generator element. */}}
{{- define "appset.cluster" -}}
{{ `{{.cluster}}` }}
{{- end -}}

{{/* Destination API server URL, from the list generator element. */}}
{{- define "appset.destServer" -}}
{{ `{{.url}}` }}
{{- end -}}

{{/* Repo-root-relative path of the matched chart directory. */}}
{{- define "appset.path" -}}
{{ `{{.path.path}}` }}
{{- end -}}

{{/* Final path segment -- cert-manager, kyverno, drone-colony-dstar, ... */}}
{{- define "appset.basename" -}}
{{ `{{.path.basename}}` }}
{{- end -}}

{{/* Sync wave, supplied per generator via the git generator's values block. */}}
{{- define "appset.wave" -}}
{{ `{{.values.wave}}` }}
{{- end -}}

{{/* Application name: <cluster>-<chart dir>, e.g. prod-cert-manager. */}}
{{- define "appset.appName" -}}
{{ `{{.cluster}}-{{.path.basename}}` }}
{{- end -}}

{{/* Namespace for cluster-apps: <cluster>-<chart dir>. */}}
{{- define "appset.appNamespace" -}}
{{ `{{.cluster}}-{{.path.basename}}` }}
{{- end -}}

{{/*
Env overlay file, resolved relative to the CHART directory (not the repo root),
which is why paths.addons / paths.apps do not appear here.
*/}}
{{- define "appset.valueFile" -}}
{{ `env/{{.cluster}}/{{.valuesFile}}` }}
{{- end -}}

{{/*
The cluster list generator elements, rendered from .Values.clusters.
Takes the ROOT context. Emitted identically by every matrix generator, so it
lives here rather than being repeated per wave group.
*/}}
{{- define "appset.clusterElements" -}}
{{- range .Values.clusters }}
- cluster: {{ .name }}
  url: {{ .url }}
  valuesFile: {{ .valuesFile }}
{{- end }}
{{- end -}}

{{/*
---------------------------------------------------------------------------
Project names.

With projects.perEnvironment true, a generated Application lands in a project
scoped to ONE destination cluster -- cluster-apps-prod permits only the prod
server. An Application generated for the wrong cluster is then rejected by
ArgoCD rather than deployed. That is the AppProject layer earning its keep.

{{.cluster}} is an ArgoCD-side placeholder, so it must survive Helm: hence the
backticks, same convention as every other placeholder in this file.

MUST agree with app-projects/values.yaml perEnvironment. hack/validate.py
enforces it -- disagreement means Applications naming projects that do not
exist, which presents as every Application stuck at "project does not exist".
---------------------------------------------------------------------------
*/}}

{{- define "appset.addonsProject" -}}
{{- if .Values.projects.perEnvironment -}}
{{ printf "%s-" .Values.projects.addons }}{{ `{{.cluster}}` }}
{{- else -}}
{{ .Values.projects.addons }}
{{- end -}}
{{- end -}}

{{- define "appset.appsProject" -}}
{{- if .Values.projects.perEnvironment -}}
{{ printf "%s-" .Values.projects.apps }}{{ `{{.cluster}}` }}
{{- else -}}
{{ .Values.projects.apps }}
{{- end -}}
{{- end -}}
