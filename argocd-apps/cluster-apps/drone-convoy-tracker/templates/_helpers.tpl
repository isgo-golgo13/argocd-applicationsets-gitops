{{/* Chart-local helpers. Every chart carries its own. */}}

{{- define "dct.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "dct.labels" -}}
app.kubernetes.io/name: {{ include "dct.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: drone-convoy-tracker
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
{{- end -}}

{{- define "dct.selectorLabels" -}}
app.kubernetes.io/name: {{ include "dct.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "dct.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}{{ include "dct.name" . }}{{- else -}}default{{- end -}}
{{- end -}}

{{- define "dct.apiImage" -}}
{{- $tag := .Values.image.api.tag | default .Chart.AppVersion -}}
{{ printf "%s/%s:%s" .Values.image.registry .Values.image.api.repository $tag }}
{{- end -}}

{{- define "dct.frontendImage" -}}
{{- $tag := .Values.image.frontend.tag | default .Chart.AppVersion -}}
{{ printf "%s/%s:%s" .Values.image.registry .Values.image.frontend.repository $tag }}
{{- end -}}

{{/*
ScyllaDB contact points. When this chart owns the cluster, the operator
publishes a client Service named <cluster>-client in the release namespace.
*/}}
{{- define "dct.scyllaHosts" -}}
{{- if .Values.scylla.create -}}
{{ printf "%s-client.%s.svc.cluster.local:9042" (include "dct.name" .) .Release.Namespace }}
{{- else -}}
{{ required "scylla.hosts is required when scylla.create is false" .Values.scylla.hosts }}
{{- end -}}
{{- end -}}
