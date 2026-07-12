import sys
import os
from PySide6.QtWidgets import (
    QApplication, QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QComboBox, QCheckBox, QLabel, QStyleFactory, QFileDialog, QPushButton
)
from PySide6.QtWebEngineWidgets import QWebEngineView
from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QPalette, QColor, QDragEnterEvent, QDropEvent

import mordant


# Comprehensive registry of common binary formats to fast-reject before reading the disk
KNOWN_BINARY_EXTENSIONS = {
    # Images
    '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp', '.tiff', '.ico', '.psd', '.ai',
    # Audio & Video
    '.mp3', '.wav', '.flac', '.aac', '.ogg', '.mp4', '.mkv', '.avi', '.mov', '.wmv', '.flv', '.webm',
    # Archives, Compression & Containers
    '.zip', '.tar', '.gz', '.bz2', '.xz', '.7z', '.rar', '.iso', '.vmdk', '.cab',
    # Executables, Libraries & Installers
    '.exe', '.dll', '.so', '.dylib', '.bin', '.elf', '.o', '.a', '.lib', '.msi', '.dmg', '.pkg', '.apk',
    # Rich Text Documents (Zip/Binary based underneath)
    '.pdf', '.docx', '.xlsx', '.pptx', '.epub', '.pages', '.numbers', '.key',
    # Database & Misc Font formats
    '.db', '.sqlite', '.sqlite3', '.ttf', '.otf', '.woff', '.woff2', '.pyc', '.wasm'
}


def is_binary_file(file_path: str, block_size: int = 1024) -> bool:
    """Performs a dual check (extension lookup + NUL byte heuristic) to detect binary files."""
    # Layer 1: Instant Extension Guard (O(1) lookups)
    _, ext = os.path.splitext(file_path)
    if ext.lower() in KNOWN_BINARY_EXTENSIONS:
        return True

    # Layer 2: Byte Heuristic Fallback (For missing, spoofed, or unknown extensions)
    try:
        with open(file_path, "rb") as f:
            chunk = f.read(block_size)
            return b"\0" in chunk
    except Exception:
        return False


class _DropWebEngineView(QWebEngineView):
    """QWebEngineView that accepts both markdown and code file drops."""

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setAcceptDrops(True)

    def dragEnterEvent(self, event: QDragEnterEvent):
        if event.mimeData().hasUrls():
            event.acceptProposedAction()

    def dropEvent(self, event: QDropEvent):
        for url in event.mimeData().urls():
            file_path = url.toLocalFile()
            if file_path:
                self._parent_window.load_file(file_path)
                break


def get_dark_palette():
    """Generates a clean dark palette for the Fusion style."""
    palette = QPalette()
    dark_gray = QColor(53, 53, 53)
    darker_gray = QColor(35, 35, 35)
    disabled_gray = QColor(127, 127, 127)

    palette.setColor(QPalette.ColorRole.Window, dark_gray)
    palette.setColor(QPalette.ColorRole.WindowText, Qt.GlobalColor.white)
    palette.setColor(QPalette.ColorRole.Base, darker_gray)
    palette.setColor(QPalette.ColorRole.AlternateBase, dark_gray)
    palette.setColor(QPalette.ColorRole.ToolTipBase, Qt.GlobalColor.white)
    palette.setColor(QPalette.ColorRole.ToolTipText, Qt.GlobalColor.white)
    palette.setColor(QPalette.ColorRole.Text, Qt.GlobalColor.white)
    palette.setColor(QPalette.ColorRole.Button, dark_gray)
    palette.setColor(QPalette.ColorRole.ButtonText, Qt.GlobalColor.white)
    palette.setColor(QPalette.ColorRole.BrightText, Qt.GlobalColor.red)
    palette.setColor(QPalette.ColorRole.Link, QColor(42, 130, 218))
    palette.setColor(QPalette.ColorRole.Highlight, QColor(42, 130, 218))
    palette.setColor(QPalette.ColorRole.HighlightedText, Qt.GlobalColor.white)

    # Disabled state — gray out text and button text so disabled widgets are visible
    palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.Text, disabled_gray)
    palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.WindowText, disabled_gray)
    palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.ButtonText, disabled_gray)
    palette.setColor(QPalette.ColorGroup.Disabled, QPalette.ColorRole.BrightText, disabled_gray)
    return palette


class MarkdownViewer(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Mordant Document Viewer")
        self.resize(1000, 750)
        self.setAcceptDrops(True)

        # State
        self.current_markdown_text = ""
        self.current_file_path = None
        self.is_code_file = False
        self.file_extension = ""

        self.init_ui()
        self.apply_theme_mode()
        self.show_welcome()

    # ------------------------------------------------------------------
    # UI Construction
    # ------------------------------------------------------------------

    def init_ui(self):
        # ── Top toolbar ──────────────────────────────────────────────
        toolbar = QWidget()
        toolbar_layout = QHBoxLayout(toolbar)
        toolbar_layout.setContentsMargins(12, 6, 12, 6)

        # File open button (emoji icon, no text)
        self.open_btn = QPushButton()
        self.open_btn.setText("\U0001F4C4")  # 📄
        self.open_btn.setFixedSize(36, 36)
        self.open_btn.setToolTip("Open file")
        self.open_btn.clicked.connect(self.open_file_dialog)
        self.open_btn.setStyleSheet(
            "QPushButton {"
            "    background: transparent;"
            "    border: none;"
            "    font-size: 20px;"
            "    padding: 0;"
            "}"
            "QPushButton:hover { background: rgba(255,255,255,0.1); border-radius: 6px; }"
        )
        toolbar_layout.addWidget(self.open_btn)

        toolbar_layout.addSpacing(12)

        # Theme selector (already sorted in the combo)
        toolbar_layout.addWidget(QLabel("Highlighting:"))
        self.theme_combo = QComboBox()
        available_themes = sorted(mordant.list_themes(), key=str.lower)
        self.theme_combo.addItems(available_themes)
        default_idx = self.theme_combo.findText("InspiredGitHub")
        if default_idx != -1:
            self.theme_combo.setCurrentIndex(default_idx)
        self.theme_combo.currentTextChanged.connect(self.update_view)
        toolbar_layout.addWidget(self.theme_combo)

        toolbar_layout.addSpacing(16)

        # Appearance mode
        toolbar_layout.addWidget(QLabel("Appearance:"))
        self.mode_combo = QComboBox()
        self.mode_combo.addItems(["Auto", "Light", "Dark"])
        self.mode_combo.currentTextChanged.connect(self.apply_theme_mode)
        toolbar_layout.addWidget(self.mode_combo)

        toolbar_layout.addSpacing(16)

        # Sync mermaid with code highlighting
        self.sync_check = QCheckBox("Sync mermaid")
        self.sync_check.setChecked(True)
        toolbar_layout.addWidget(self.sync_check)

        # Mermaid native theme dropdown
        self.mermaid_label = QLabel("Mermaid:")
        toolbar_layout.addWidget(self.mermaid_label)
        self.mermaid_combo = QComboBox()
        self.mermaid_combo.addItems(["Default", "modern", "dark", "forest", "neutral"])
        self.mermaid_combo.setCurrentIndex(0)  # "Default" = no override
        self.mermaid_combo.currentTextChanged.connect(self.update_view)
        self.sync_check.toggled.connect(self.on_sync_changed)
        toolbar_layout.addWidget(self.mermaid_combo)

        toolbar_layout.addSpacing(16)

        # Math output format
        toolbar_layout.addWidget(QLabel("Math:"))
        self.math_combo = QComboBox()
        self.math_combo.addItems(["both", "html", "mathml"])
        self.math_combo.setCurrentText("both")
        self.math_combo.currentTextChanged.connect(self.update_view)
        toolbar_layout.addWidget(self.math_combo)

        toolbar_layout.addStretch()

        # ── Web view (also handles drag & drop) ──────────────────────
        self.webview = _DropWebEngineView()
        self.webview._parent_window = self

        # ── Assemble ─────────────────────────────────────────────────
        central = QWidget()
        main_layout = QVBoxLayout(central)
        main_layout.setContentsMargins(0, 0, 0, 0)
        main_layout.setSpacing(0)

        main_layout.addWidget(toolbar)
        main_layout.addWidget(self.webview, 1)

        self.setCentralWidget(central)

        # Force initial disabled state for mermaid group (sync is checked by default)
        self.on_sync_changed(self.sync_check.isChecked())

        QApplication.styleHints().colorSchemeChanged.connect(self.on_system_theme_changed)

    # ------------------------------------------------------------------
    # File I/O
    # ------------------------------------------------------------------

    def open_file_dialog(self):
        dialog = QFileDialog(self, "Open Document or Code File")
        dialog.setFileMode(QFileDialog.FileMode.ExistingFile)
        dialog.setNameFilter("All Files (*);;Markdown (*.md *.markdown);;Source Files (*.py *.js *.ts *.rs *.go *.c *.cpp *.html *.css *.json *.yaml)")
        if dialog.exec() == QFileDialog.Accepted:
            path = dialog.selectedFiles()[0]
            self.load_file(path)

    def load_file(self, file_path: str):
        # 1. Binary sanity check guard boundary
        if is_binary_file(file_path):
            self.current_markdown_text = (
                f"### ❌ File load rejected\n\n"
                f"`{os.path.basename(file_path)}` appears to be a **binary format** or utilizes incompatible encoding variants."
            )
            self.current_file_path = file_path
            self.is_code_file = False
            self.setWindowTitle(f"Error Viewing — {os.path.basename(file_path)}")
            self.update_view()
            return

        try:
            with open(file_path, "r", encoding="utf-8") as f:
                self.current_markdown_text = f.read()
            
            self.current_file_path = file_path
            ext = os.path.splitext(file_path)[1].lower()

            # 2. File type classification rules
            if ext in ('.md', '.markdown'):
                self.is_code_file = False
                self.file_extension = ""
            else:
                self.is_code_file = True
                self.file_extension = ext.lstrip('.')

            self.setWindowTitle(
                f"Mordant Document Viewer — {os.path.basename(file_path)}"
            )
            self.update_view()
        except Exception as err:
            self.current_markdown_text = f"__Error loading file:__ `{err}`"
            self.is_code_file = False
            self.update_view()

    def show_welcome(self):
        self.current_markdown_text = (
            "## 👋 Welcome to Mordant Document Viewer\n\n"
            "Drag & drop an `.md` markdown document or a **source code file** here to begin.\n\n"
            "The system uses the Rust-powered **Mordant** compiler engine to process "
            "CommonMark layouts, structural code blocks, and language syntax layouts[cite: 2]."
        )
        self.current_file_path = None
        self.is_code_file = False
        self.file_extension = ""
        self.setWindowTitle("Mordant Document Viewer")
        self.update_view()

    # ------------------------------------------------------------------
    # Theming
    # ------------------------------------------------------------------

    def is_dark_mode_active(self) -> bool:
        mode = self.mode_combo.currentText()
        if mode == "Dark":
            return True
        if mode == "Light":
            return False
        return QApplication.styleHints().colorScheme() == Qt.ColorScheme.Dark

    def apply_theme_mode(self):
        is_dark = self.is_dark_mode_active()
        if is_dark:
            QApplication.setPalette(get_dark_palette())
        else:
            QApplication.setPalette(QApplication.style().standardPalette())
        self.update_view()

    def on_system_theme_changed(self):
        if self.mode_combo.currentText() == "Auto":
            self.apply_theme_mode()

    def on_sync_changed(self, checked: bool):
        """Enable/disable the mermaid theme dropdown when sync toggle changes."""
        self.mermaid_label.setEnabled(not checked)
        self.mermaid_combo.setEnabled(not checked)
        self.update_view()

    # ------------------------------------------------------------------
    # Rendering
    # ------------------------------------------------------------------

    def update_view(self):
        if not self.current_markdown_text:
            return

        is_dark = self.is_dark_mode_active()
        bg_color = "#232323" if is_dark else "#ffffff"
        text_color = "#ffffff" if is_dark else "#000000"
        accent_color = "#353535" if is_dark else "#f6f8fa"
        border_color = "#444444" if is_dark else "#d0d7de"
        scheme = "dark" if is_dark else "light"

        selected_highlight = self.theme_combo.currentText()

        # 3. Dynamic Parser Dispatch Block
        if self.is_code_file:
            try:
                # Initialize mordant highlighter with selected parameters
                highlighter = mordant.Highlighter(theme=selected_highlight, mode="Attribute")
                # If extension matching yields blank string, passing it natively invokes mordant's cascading content-heuristics
                html_body = highlighter.highlight(self.file_extension, self.current_markdown_text)
            except Exception as err:
                html_body = f"<p style='color:red;'>Failed to compile syntax tree: {err}</p>"
        else:
            try:
                gfm = mordant.GfmOptions()

                # Determine the mermaid diagram theme
                if self.sync_check.isChecked():
                    # Sync with code highlighting: use the same them name
                    diag_opts = mordant.PyDiagramHtmlRendererOptions(theme=selected_highlight)
                else:
                    mermaid_theme = self.mermaid_combo.currentText()
                    # "Default" maps to mermaid's built-in "default" theme
                    theme_name = "default" if mermaid_theme == "Default" else mermaid_theme
                    diag_opts = mordant.PyDiagramHtmlRendererOptions(theme=theme_name)

                math_output = self.math_combo.currentText()
                math_opts = mordant.PyMathRendererOptions(output=math_output)

                html_body = mordant.markdown_to_html(
                    self.current_markdown_text,
                    gfm_opts=gfm,
                    highlighting_theme=selected_highlight,
                    highlighting_mode="Attribute",
                    diagram_render_opts=diag_opts,
                    math_renderer_opts=math_opts,
                )
            except Exception as err:
                html_body = f"<p style='color:red;'>Failed to parse: {err}</p>"

        full_html = f"""
        <!DOCTYPE html>
        <html>
        <head>
        <meta charset="utf-8">
        <style>{mordant.KATEX_CSS}</style>
        <style>
            :root {{
                color-scheme: {scheme};
            }}
            body {{
                background-color: {bg_color};
                color: {text_color};
                font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
                padding: 30px;
                line-height: 1.6;
                max-width: 850px;
                margin: 0 auto;
            }}
            pre {{
                /* Background is supplied by the selected syntax theme
                   (mordant sets it inline on <pre>); do not override it here. */
                border: 1px solid {border_color} !important;
                padding: 16px !important;
                border-radius: 6px !important;
                overflow: auto !important;
            }}
            code {{
                font-family: ui-monospace, SFMono-Regular, SF Mono, Menlo, Consolas, Liberation Mono, monospace;
                font-size: 85%;
            }}
            p > code, li > code {{
                background-color: {accent_color};
                padding: .2em .4em;
                border-radius: 6px;
            }}
            table {{
                border-collapse: collapse;
                width: 100%;
                margin-top: 0;
                margin-bottom: 16px;
            }}
            th, td {{
                border: 1px solid {border_color};
                padding: 6px 13px;
            }}
            tr:nth-child(2n) {{
                background-color: {accent_color};
            }}
            blockquote {{
                padding: 0 1em;
                color: #57606a;
                border-left: .25em solid {border_color};
                margin: 0;
            }}
            a {{
                color: #0969da;
                text-decoration: none;
            }}
            a:hover {{
                text-decoration: underline;
            }}
            ::-webkit-scrollbar-track {{
                background: {bg_color};
            }}
            ::-webkit-scrollbar-thumb {{
                background: {border_color};
                border-radius: 5px;
            }}
            ::-webkit-scrollbar-thumb:hover {{
                background: {accent_color};
            }}
            * {{
                scrollbar-color: {border_color} {bg_color};
                scrollbar-width: auto;
            }}
        </style>
        </head>
        <body>
            {html_body}
        </body>
        </html>
        """
        self.webview.setHtml(full_html)


if __name__ == "__main__":
    app = QApplication(sys.argv)
    app.setStyle(QStyleFactory.create("Fusion"))

    viewer = MarkdownViewer()
    viewer.show()
    sys.exit(app.exec())
