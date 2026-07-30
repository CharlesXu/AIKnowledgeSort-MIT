import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";

export type UiLanguage = "en" | "zh-CN";

const STORAGE_KEY = "aiks.ui.language.v1";

const en = {
  "app.header": "Application header",
  "app.local": "Local",
  "app.addSource": "Add source",
  "app.addSourceHint": "Add local files or folders",
  "app.addFiles": "Add files…",
  "app.addFolders": "Add folders…",
  "tools.label": "Workbench tools",
  "tools.sources": "Sources",
  "tools.search": "Search",
  "tools.graph": "Graph",
  "tools.classification": "Classification",
  "tools.archive": "Archive",
  "tools.settings": "Settings",
  "settings.kicker": "LOCAL AUTHORITY",
  "settings.title": "Settings",
  "settings.close": "Close settings",
  "settings.sections": "Settings sections",
  "settings.models": "Model runtime",
  "settings.agents": "Agent access",
  "settings.language": "Language",
  "language.title": "Interface language",
  "language.description": "Choose the language used by the local desktop interface.",
  "language.english": "English",
  "language.chinese": "简体中文",
  "models.configured": "Configured models",
  "models.none": "No model configurations.",
  "models.protocol": "Protocol",
  "models.openai": "OpenAI compatible",
  "models.anthropic": "Anthropic",
  "models.credentialEnvironment": "Credential environment",
  "models.credentialStored": "API key stored in system credential vault",
  "models.edit": "Edit",
  "models.remove": "Remove",
  "models.addEdit": "Add or edit",
  "models.configId": "Configuration ID",
  "models.label": "Label",
  "models.location": "Location",
  "models.local": "Local",
  "models.remote": "Remote",
  "models.endpoint": "Endpoint URL",
  "models.model": "Model",
  "models.refresh": "Refresh models",
  "models.refreshing": "Refreshing…",
  "models.detected": "Detected",
  "models.timeout": "Timeout (seconds)",
  "models.useAuth": "Use bearer authentication",
  "models.credentialSource": "Credential source",
  "models.environment": "Environment variable",
  "models.keychain": "System credential vault",
  "models.credentialVariable": "Credential environment variable",
  "models.configIdPlaceholder": "Enter a configuration ID first",
  "models.environmentHint": "Set the token in this environment variable before launching the app.",
  "models.apiKey": "API Key",
  "models.keyStoredPlaceholder": "Stored — enter to replace",
  "models.keyPlaceholder": "Enter API key",
  "models.keyHint": "The key is stored by the operating system and never written to app JSON.",
  "models.save": "Save model config",
  "models.saving": "Saving…",
  "models.runtimeKicker": "LOCAL RUNTIME",
  "models.runtimeTitle": "Model runtime settings",
  "models.close": "Close model settings",
  "sources.panel": "Sources",
  "sources.local": "LOCAL",
  "sources.files": "FILES",
  "sources.expand": "Expand Sources panel",
  "sources.collapse": "Collapse Sources panel",
  "sources.search": "Search sources",
  "sources.filter": "Filter files and folders",
  "sources.localFolders": "Local source folders",
  "sources.noMatch": "No sources match “{query}”",
  "sources.selectedOne": "{count} unique eligible file selected",
  "sources.selectedMany": "{count} unique eligible files selected",
  "scan.report": "Scan report",
  "scan.demo": "Demo scan",
  "scan.live": "Live scan",
  "scan.browserFixture": "Browser fixture",
  "scan.trustedResult": "Trusted local result",
  "scan.previewed": "{count} previewed",
  "scan.counts": "Discovery counts",
  "scan.included": "Included",
  "scan.excluded": "Excluded",
  "scan.unreadable": "Unreadable",
  "scan.symlinks": "Symlinks",
  "scan.outOfScope": "Out of scope",
  "scan.dropStatus": "Drop status",
  "scan.unchanged": "No files have been changed",
  "scan.dropHint": "Drop files or folders anywhere to scan.",
  "shell.workbench": "Source workbench",
  "shell.workspace": "Knowledge workspace",
  "shell.resizeSources": "Resize Sources panel",
  "shell.resizeContext": "Resize import review context",
  "shell.dropTarget": "Native drop target",
  "shell.release": "Release to review",
  "shell.releaseHint": "Paths stay native; discovery starts only after a trusted grant.",
  "shell.demoWorkspace": "Local demo workspace",
  "shell.trustedProposal": "Trusted local proposal",
  "shell.readOnly": "Read-only scan report",
  "shell.eligible": "{count} eligible · 0 changes",
} as const;

export type TranslationKey = keyof typeof en;

const zhCN: Record<TranslationKey, string> = {
  "app.header": "应用标题栏",
  "app.local": "本地",
  "app.addSource": "添加来源",
  "app.addSourceHint": "添加本地文件或文件夹",
  "app.addFiles": "添加文件…",
  "app.addFolders": "添加文件夹…",
  "tools.label": "工作区工具",
  "tools.sources": "来源",
  "tools.search": "搜索",
  "tools.graph": "图谱",
  "tools.classification": "分类",
  "tools.archive": "归档",
  "tools.settings": "设置",
  "settings.kicker": "本地权限域",
  "settings.title": "设置",
  "settings.close": "关闭设置",
  "settings.sections": "设置分区",
  "settings.models": "模型运行时",
  "settings.agents": "Agent 访问",
  "settings.language": "语言",
  "language.title": "界面语言",
  "language.description": "选择本地桌面界面使用的语言。",
  "language.english": "English",
  "language.chinese": "简体中文",
  "models.configured": "已配置模型",
  "models.none": "尚未配置模型。",
  "models.protocol": "协议",
  "models.openai": "OpenAI 兼容",
  "models.anthropic": "Anthropic",
  "models.credentialEnvironment": "凭据环境变量",
  "models.credentialStored": "API Key 已存入系统凭据库",
  "models.edit": "编辑",
  "models.remove": "删除",
  "models.addEdit": "添加或编辑",
  "models.configId": "配置 ID",
  "models.label": "名称",
  "models.location": "位置",
  "models.local": "本地",
  "models.remote": "远程",
  "models.endpoint": "模型 URL",
  "models.model": "模型",
  "models.refresh": "刷新模型列表",
  "models.refreshing": "刷新中…",
  "models.detected": "已识别",
  "models.timeout": "超时（秒）",
  "models.useAuth": "使用 API 鉴权",
  "models.credentialSource": "凭据来源",
  "models.environment": "环境变量",
  "models.keychain": "系统凭据库",
  "models.credentialVariable": "凭据环境变量",
  "models.configIdPlaceholder": "请先输入配置 ID",
  "models.environmentHint": "请在启动应用前将令牌写入该环境变量。",
  "models.apiKey": "API Key",
  "models.keyStoredPlaceholder": "已保存——输入新值可替换",
  "models.keyPlaceholder": "输入 API Key",
  "models.keyHint": "密钥由操作系统安全保存，不会写入应用配置 JSON。",
  "models.save": "保存模型配置",
  "models.saving": "保存中…",
  "models.runtimeKicker": "本地运行时",
  "models.runtimeTitle": "模型运行时设置",
  "models.close": "关闭模型设置",
  "sources.panel": "来源",
  "sources.local": "本地",
  "sources.files": "个文件",
  "sources.expand": "展开来源面板",
  "sources.collapse": "折叠来源面板",
  "sources.search": "搜索来源",
  "sources.filter": "筛选文件和文件夹",
  "sources.localFolders": "本地来源目录",
  "sources.noMatch": "没有来源匹配“{query}”",
  "sources.selectedOne": "已选择 {count} 个符合条件的唯一文件",
  "sources.selectedMany": "已选择 {count} 个符合条件的唯一文件",
  "scan.report": "扫描报告",
  "scan.demo": "演示扫描",
  "scan.live": "实时扫描",
  "scan.browserFixture": "浏览器演示数据",
  "scan.trustedResult": "可信本地结果",
  "scan.previewed": "已预览 {count} 项",
  "scan.counts": "发现统计",
  "scan.included": "纳入",
  "scan.excluded": "排除",
  "scan.unreadable": "不可读",
  "scan.symlinks": "符号链接",
  "scan.outOfScope": "范围外",
  "scan.dropStatus": "拖放状态",
  "scan.unchanged": "尚未更改任何文件",
  "scan.dropHint": "可将文件或文件夹拖到任意位置进行扫描。",
  "shell.workbench": "来源工作区",
  "shell.workspace": "知识工作区",
  "shell.resizeSources": "调整来源面板宽度",
  "shell.resizeContext": "调整导入审查面板宽度",
  "shell.dropTarget": "原生拖放目标",
  "shell.release": "松开以审查",
  "shell.releaseHint": "路径保持原生；仅在获得可信授权后开始发现。",
  "shell.demoWorkspace": "本地演示工作区",
  "shell.trustedProposal": "可信本地提案",
  "shell.readOnly": "只读扫描报告",
  "shell.eligible": "{count} 项符合条件 · 0 项更改",
};

interface I18nValue {
  readonly language: UiLanguage;
  readonly setLanguage: (language: UiLanguage) => void;
  readonly t: (key: TranslationKey, values?: Readonly<Record<string, string | number>>) => string;
}

function detectedLanguage(): UiLanguage {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  if (stored === "en" || stored === "zh-CN") return stored;
  return window.navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

const defaultValue: I18nValue = {
  language: "en",
  setLanguage: () => undefined,
  t: (key, values) => interpolate(en[key], values),
};

const I18nContext = createContext<I18nValue>(defaultValue);

function interpolate(
  template: string,
  values: Readonly<Record<string, string | number>> = {},
): string {
  return Object.entries(values).reduce(
    (result, [name, value]) => result.replaceAll(`{${name}}`, String(value)),
    template,
  );
}

export function I18nProvider({
  children,
  initialLanguage,
}: {
  readonly children: React.ReactNode;
  readonly initialLanguage?: UiLanguage;
}) {
  const [language, setLanguageState] = useState<UiLanguage>(
    () => initialLanguage ?? detectedLanguage(),
  );
  const setLanguage = useCallback((next: UiLanguage) => {
    window.localStorage.setItem(STORAGE_KEY, next);
    setLanguageState(next);
  }, []);
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  const value = useMemo<I18nValue>(() => ({
    language,
    setLanguage,
    t: (key, values) => interpolate(
      language === "zh-CN" ? zhCN[key] : en[key],
      values,
    ),
  }), [language, setLanguage]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}
