/* Буст от nova: нативный модуль на C.
   Очистка оперативной памяти, список окон программ, мягкое закрытие, проверка прав админа.
   Вызывается из Rust (sys.rs). Работает только на Windows. */
#define _CRT_SECURE_NO_WARNINGS
#define WIN32_LEAN_AND_MEAN
#define PSAPI_VERSION 2
#include <windows.h>
#include <tlhelp32.h>
#include <psapi.h>
#include <stdint.h>
#include <string.h>
#include <wchar.h>

typedef struct {
    uint32_t pid;
    uint32_t ws_mb;
    wchar_t name[64];
    wchar_t title[128];
} BoostApp;

/* ---------- память ---------- */

int boost_ram_mb(uint64_t *total, uint64_t *avail) {
    MEMORYSTATUSEX m;
    m.dwLength = sizeof(m);
    if (!GlobalMemoryStatusEx(&m)) {
        return 0;
    }
    *total = (uint64_t)(m.ullTotalPhys / 1048576ULL);
    *avail = (uint64_t)(m.ullAvailPhys / 1048576ULL);
    return 1;
}

static void enable_priv(const wchar_t *name) {
    HANDLE tok;
    TOKEN_PRIVILEGES tp;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &tok)) {
        return;
    }
    tp.PrivilegeCount = 1;
    if (LookupPrivilegeValueW(NULL, name, &tp.Privileges[0].Luid)) {
        tp.Privileges[0].Attributes = SE_PRIVILEGE_ENABLED;
        AdjustTokenPrivileges(tok, FALSE, &tp, sizeof(tp), NULL, NULL);
    }
    CloseHandle(tok);
}

/* Сжимает рабочий набор всех процессов (страницы уходят из «занятой» памяти). Возвращает число процессов. */
int boost_trim_all(void) {
    HANDLE snap;
    PROCESSENTRY32W pe;
    int n = 0;
    enable_priv(L"SeDebugPrivilege");
    snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if (snap == INVALID_HANDLE_VALUE) {
        return 0;
    }
    pe.dwSize = sizeof(pe);
    if (Process32FirstW(snap, &pe)) {
        do {
            HANDLE h;
            if (pe.th32ProcessID == 0 || pe.th32ProcessID == 4) {
                continue;
            }
            h = OpenProcess(PROCESS_SET_QUOTA | PROCESS_QUERY_INFORMATION, FALSE, pe.th32ProcessID);
            if (h) {
                if (EmptyWorkingSet(h)) {
                    n++;
                }
                CloseHandle(h);
            }
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
    return n;
}

typedef LONG (NTAPI *NtSetSysInfoFn)(ULONG, PVOID, ULONG);

/* Сбрасывает кэш Windows (список ожидания и изменённые страницы). Нужны права администратора.
   Возвращает битовую маску удавшихся шагов (0 = ничего не вышло). */
int boost_purge_standby(void) {
    HMODULE nt = GetModuleHandleW(L"ntdll.dll");
    NtSetSysInfoFn fn;
    ULONG cmd;
    int ok = 0;
    if (!nt) {
        return 0;
    }
    fn = (NtSetSysInfoFn)GetProcAddress(nt, "NtSetSystemInformation");
    if (!fn) {
        return 0;
    }
    enable_priv(L"SeProfileSingleProcessPrivilege");
    cmd = 2; /* MemoryEmptyWorkingSets */
    if (fn(80, &cmd, sizeof(cmd)) >= 0) {
        ok |= 1;
    }
    cmd = 3; /* MemoryFlushModifiedList */
    if (fn(80, &cmd, sizeof(cmd)) >= 0) {
        ok |= 2;
    }
    cmd = 4; /* MemoryPurgeStandbyList */
    if (fn(80, &cmd, sizeof(cmd)) >= 0) {
        ok |= 4;
    }
    return ok;
}

/* ---------- список программ с окнами ---------- */

static const wchar_t *PROTECTED[] = {
    L"explorer.exe", L"dwm.exe", L"csrss.exe", L"winlogon.exe", L"services.exe", L"lsass.exe",
    L"svchost.exe", L"sihost.exe", L"taskhostw.exe", L"ctfmon.exe", L"searchhost.exe",
    L"searchapp.exe", L"shellexperiencehost.exe", L"startmenuexperiencehost.exe",
    L"textinputhost.exe", L"lockapp.exe", L"securityhealthsystray.exe", L"msmpeng.exe",
    L"runtimebroker.exe", L"fontdrvhost.exe", L"applicationframehost.exe", L"systemsettings.exe",
    L"boost.exe", L"bnova.exe", NULL
};

static int is_protected(const wchar_t *name) {
    int i;
    for (i = 0; PROTECTED[i]; i++) {
        if (_wcsicmp(name, PROTECTED[i]) == 0) {
            return 1;
        }
    }
    return 0;
}

static int proc_name(DWORD pid, wchar_t *out, int cap) {
    HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    wchar_t path[MAX_PATH];
    DWORD len = MAX_PATH;
    int ok = 0;
    if (!h) {
        return 0;
    }
    if (QueryFullProcessImageNameW(h, 0, path, &len)) {
        wchar_t *b = wcsrchr(path, L'\\');
        b = b ? b + 1 : path;
        wcsncpy(out, b, cap - 1);
        out[cap - 1] = 0;
        ok = 1;
    }
    CloseHandle(h);
    return ok;
}

typedef HRESULT (WINAPI *DwmGetAttrFn)(HWND, DWORD, PVOID, DWORD);

static int is_cloaked(HWND h) {
    static DwmGetAttrFn fn = NULL;
    static int tried = 0;
    DWORD cloaked = 0;
    if (!tried) {
        HMODULE m;
        tried = 1;
        m = LoadLibraryW(L"dwmapi.dll");
        if (m) {
            fn = (DwmGetAttrFn)GetProcAddress(m, "DwmGetWindowAttribute");
        }
    }
    if (!fn) {
        return 0;
    }
    if (fn(h, 14, &cloaked, sizeof(cloaked)) == S_OK) { /* 14 = DWMWA_CLOAKED */
        return cloaked != 0;
    }
    return 0;
}

typedef struct {
    BoostApp *out;
    int max;
    int n;
    DWORD self;
} EnumCtx;

static BOOL CALLBACK enum_cb(HWND h, LPARAM lp) {
    EnumCtx *c = (EnumCtx *)lp;
    LONG_PTR ex;
    wchar_t title[128];
    wchar_t name[64];
    DWORD pid = 0;
    int i;
    BoostApp *a;
    if (!IsWindowVisible(h)) {
        return TRUE;
    }
    if (GetWindow(h, GW_OWNER) != NULL) {
        return TRUE;
    }
    ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
    if ((ex & WS_EX_TOOLWINDOW) && !(ex & WS_EX_APPWINDOW)) {
        return TRUE;
    }
    if (is_cloaked(h)) {
        return TRUE;
    }
    if (GetWindowTextW(h, title, 128) <= 0) {
        return TRUE;
    }
    GetWindowThreadProcessId(h, &pid);
    if (pid == 0 || pid == c->self) {
        return TRUE;
    }
    for (i = 0; i < c->n; i++) {
        if (c->out[i].pid == pid) {
            return TRUE;
        }
    }
    if (c->n >= c->max) {
        return TRUE;
    }
    if (!proc_name(pid, name, 64)) {
        return TRUE;
    }
    if (is_protected(name)) {
        return TRUE;
    }
    a = &c->out[c->n];
    c->n++;
    memset(a, 0, sizeof(*a));
    a->pid = (uint32_t)pid;
    wcsncpy(a->name, name, 63);
    wcsncpy(a->title, title, 127);
    return TRUE;
}

/* Суммарная память всех процессов с таким же именем (у браузеров их много). */
static uint32_t total_ws_mb(const wchar_t *name) {
    HANDLE snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    PROCESSENTRY32W pe;
    uint64_t sum = 0;
    if (snap == INVALID_HANDLE_VALUE) {
        return 0;
    }
    pe.dwSize = sizeof(pe);
    if (Process32FirstW(snap, &pe)) {
        do {
            if (_wcsicmp(pe.szExeFile, name) == 0) {
                HANDLE h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pe.th32ProcessID);
                if (h) {
                    PROCESS_MEMORY_COUNTERS pmc;
                    if (GetProcessMemoryInfo(h, &pmc, sizeof(pmc))) {
                        sum += (uint64_t)pmc.WorkingSetSize;
                    }
                    CloseHandle(h);
                }
            }
        } while (Process32NextW(snap, &pe));
    }
    CloseHandle(snap);
    return (uint32_t)(sum / 1048576ULL);
}

int boost_list_apps(BoostApp *out, int max) {
    EnumCtx c;
    int i;
    c.out = out;
    c.max = max;
    c.n = 0;
    c.self = GetCurrentProcessId();
    EnumWindows(enum_cb, (LPARAM)&c);
    for (i = 0; i < c.n; i++) {
        out[i].ws_mb = total_ws_mb(out[i].name);
    }
    return c.n;
}

/* ---------- закрытие ---------- */

static BOOL CALLBACK close_cb(HWND h, LPARAM lp) {
    DWORD pid = 0;
    GetWindowThreadProcessId(h, &pid);
    if (pid == (DWORD)lp && IsWindowVisible(h)) {
        PostMessageW(h, WM_CLOSE, 0, 0);
    }
    return TRUE;
}

/* Мягко просит окна процесса закрыться (как крестик). Программа сама спросит про сохранение. */
int boost_close_pid(uint32_t pid) {
    EnumWindows(close_cb, (LPARAM)pid);
    return 1;
}

int boost_is_alive(uint32_t pid) {
    HANDLE h = OpenProcess(SYNCHRONIZE, FALSE, pid);
    DWORD r;
    if (!h) {
        return 0;
    }
    r = WaitForSingleObject(h, 0);
    CloseHandle(h);
    return r == WAIT_TIMEOUT;
}

int boost_kill_pid(uint32_t pid) {
    HANDLE h = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
    int ok;
    if (!h) {
        return 0;
    }
    ok = TerminateProcess(h, 1) ? 1 : 0;
    CloseHandle(h);
    return ok;
}

/* ---------- права ---------- */

int boost_is_admin(void) {
    HANDLE tok;
    TOKEN_ELEVATION e;
    DWORD sz = 0;
    int r = 0;
    if (OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &tok)) {
        if (GetTokenInformation(tok, TokenElevation, &e, sizeof(e), &sz)) {
            r = e.TokenIsElevated ? 1 : 0;
        }
        CloseHandle(tok);
    }
    return r;
}
