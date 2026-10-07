'use strict';
// blessed-core.js — faithful port of the blessed 0.1.81 subset vtop needs.
// Sources (ORACLE, byte-by-byte 2026-10-05):
//   .../blessed/lib/colors.js           (vcolors, match, convert)
//   .../blessed/lib/widgets/element.js  (sattr, _parseTags, _parseAttr, _wrapContent,
//                                        _getCoords, render, borders, labels)
//   .../blessed/lib/widgets/list.js     (createItem, setItems, select, scrollTo)
//   .../blessed/lib/widgets/screen.js   (attrCode, draw, alloc, render)
//   .../blessed/lib/program.js          (_attr SGR table)
// Deviations for capture-only use: program output goes to a string buffer,
// no mouse/keys/input handling, no CSR/cuf/BCE/ACS optimizations (CONTRACTS C9).

var colors = require('./blessed-colors');

// ---------------------------------------------------------------------------
// colors (verbatim from lib/colors.js — see blessed-colors.js, required above)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Program._attr: SGR table (program.js 0.1.81, subset used by vtop tags)
// ---------------------------------------------------------------------------
var SGR = {
  'normal':         { 'true': '\x1b[m',   'false': '' },
  'default':        { 'true': '\x1b[m',   'false': '' },
  'bold':           { 'true': '\x1b[1m',  'false': '\x1b[22m' },
  'ul':             { 'true': '\x1b[4m',  'false': '\x1b[24m' },
  'underline':      { 'true': '\x1b[4m',  'false': '\x1b[24m' },
  'underlined':     { 'true': '\x1b[4m',  'false': '\x1b[24m' },
  'blink':          { 'true': '\x1b[5m',  'false': '\x1b[25m' },
  'inverse':        { 'true': '\x1b[7m',  'false': '\x1b[27m' },
  'invisible':      { 'true': '\x1b[8m',  'false': '\x1b[28m' },
  'black fg':       { 'true': '\x1b[30m', 'false': '\x1b[39m' },
  'red fg':         { 'true': '\x1b[31m', 'false': '\x1b[39m' },
  'green fg':       { 'true': '\x1b[32m', 'false': '\x1b[39m' },
  'yellow fg':      { 'true': '\x1b[33m', 'false': '\x1b[39m' },
  'blue fg':        { 'true': '\x1b[34m', 'false': '\x1b[39m' },
  'magenta fg':     { 'true': '\x1b[35m', 'false': '\x1b[39m' },
  'cyan fg':        { 'true': '\x1b[36m', 'false': '\x1b[39m' },
  'white fg':       { 'true': '\x1b[37m', 'false': '\x1b[39m' },
  'light grey fg':  { 'true': '\x1b[37m', 'false': '\x1b[39m' },
  'light gray fg':  { 'true': '\x1b[37m', 'false': '\x1b[39m' },
  'bright grey fg': { 'true': '\x1b[37m', 'false': '\x1b[39m' },
  'bright gray fg': { 'true': '\x1b[37m', 'false': '\x1b[39m' },
  'default fg':     { 'true': '\x1b[39m', 'false': '' },
  'black bg':       { 'true': '\x1b[40m', 'false': '\x1b[49m' },
  'red bg':         { 'true': '\x1b[41m', 'false': '\x1b[49m' },
  'green bg':       { 'true': '\x1b[42m', 'false': '\x1b[49m' },
  'yellow bg':      { 'true': '\x1b[43m', 'false': '\x1b[49m' },
  'blue bg':        { 'true': '\x1b[44m', 'false': '\x1b[49m' },
  'magenta bg':     { 'true': '\x1b[45m', 'false': '\x1b[49m' },
  'cyan bg':        { 'true': '\x1b[46m', 'false': '\x1b[49m' },
  'white bg':       { 'true': '\x1b[47m', 'false': '\x1b[49m' },
  'default bg':     { 'true': '\x1b[49m', 'false': '' },
  'light black bg':   { 'true': '\x1b[100m', 'false': '\x1b[49m' },
  'light red bg':     { 'true': '\x1b[101m', 'false': '\x1b[49m' },
  'light green bg':   { 'true': '\x1b[102m', 'false': '\x1b[49m' },
  'light yellow bg':  { 'true': '\x1b[103m', 'false': '\x1b[49m' },
  'light blue bg':    { 'true': '\x1b[104m', 'false': '\x1b[49m' },
  'light magenta bg': { 'true': '\x1b[105m', 'false': '\x1b[49m' },
  'light cyan bg':    { 'true': '\x1b[106m', 'false': '\x1b[49m' },
  'light white bg':   { 'true': '\x1b[107m', 'false': '\x1b[49m' }
  // 256-color fg/bg handled separately below (tag parser resolves names first)
};

function programAttr(param, val) {
  param = param || 'normal';
  param = param.replace(/[\-]/g, ' ');
  var parts = param.split(/\s*[,;]\s*/);
  if (parts.length > 1) {
    var used = {}
      , out = [];
    parts.forEach(function(part) {
      var p = programAttr(part, val).slice(2, -1);
      if (p === '') return;
      if (used[p]) return;
      used[p] = true;
      out.push(p);
    });
    return '\x1b[' + out.join(';') + 'm';
  }
  if (param.indexOf('no ') === 0) {
    param = param.substring(3);
    val = false;
  } else if (param.indexOf('!') === 0) {
    param = param.substring(1);
    val = false;
  }
  var hit = SGR[param];
  if (hit) return hit[val === false ? 'false' : 'true'];
  // color name lookup: "<color> fg" / "<color> bg" -> 38;5;n / 48;5;n
  var m = /^([\w\- ]+) (fg|bg)$/.exec(param)
    , color
    , sgr;
  if (m) {
    color = m[1].replace(/[\- ]/g, '');
    sgr = m[2] === 'fg' ? 38 : 48;
    if (colorNames[color] != null) {
      var c = colorNames[color];
      if (c === -1) {
        return val === false ? '' : (m[2] === 'fg' ? '\x1b[39m' : '\x1b[49m');
      }
      if (c < 16) {
        if (m[2] === 'fg') {
          return val === false ? '\x1b[39m' : (c < 8 ? '\x1b[' + (30 + c) + 'm' : '\x1b[' + (90 + c - 8) + 'm');
        }
        return val === false ? '\x1b[49m' : (c < 8 ? '\x1b[' + (40 + c) + 'm' : '\x1b[' + (100 + c - 8) + 'm');
      }
      if (val === false) {
        return m[2] === 'fg' ? '\x1b[39m' : '\x1b[49m';
      }
      return '\x1b[' + sgr + ';5;' + c + 'm';
    }
  }
  return '';
}

var colorNames = colors.colorNames;

// ---------------------------------------------------------------------------
// tput helpers — ansi cup string: '\x1b[%i%p1%d;%p2%dH'
// ---------------------------------------------------------------------------
function cup(y, x) {
  return '\x1b[' + (y + 1) + ';' + (x + 1) + 'H';
}

var DATTR = ((0 << 18) | (0x1ff << 9)) | 0x1ff;

// ---------------------------------------------------------------------------
// attrCode — verbatim from screen.js (SGR string -> attr int)
// ---------------------------------------------------------------------------
function attrCode(code, cur, def) {
  var flags = (cur >> 18) & 0x1ff
    , fg = (cur >> 9) & 0x1ff
    , bg = cur & 0x1ff
    , c
    , i;

  code = code.slice(2, -1).split(';');
  if (!code[0]) code[0] = '0';

  for (i = 0; i < code.length; i++) {
    c = +code[i] || 0;
    switch (c) {
      case 0:
        bg = def & 0x1ff;
        fg = (def >> 9) & 0x1ff;
        flags = (def >> 18) & 0x1ff;
        break;
      case 1:
        flags |= 1;
        break;
      case 22:
        flags = (def >> 18) & 0x1ff;
        break;
      case 4:
        flags |= 2;
        break;
      case 24:
        flags = (def >> 18) & 0x1ff;
        break;
      case 5:
        flags |= 4;
        break;
      case 25:
        flags = (def >> 18) & 0x1ff;
        break;
      case 7:
        flags |= 8;
        break;
      case 27:
        flags = (def >> 18) & 0x1ff;
        break;
      case 8:
        flags |= 16;
        break;
      case 28:
        flags = (def >> 18) & 0x1ff;
        break;
      case 39:
        fg = (def >> 9) & 0x1ff;
        break;
      case 49:
        bg = def & 0x1ff;
        break;
      case 100:
        fg = (def >> 9) & 0x1ff;
        bg = def & 0x1ff;
        break;
      default:
        if (c === 48 && +code[i + 1] === 5) {
          i += 2;
          bg = +code[i];
          break;
        } else if (c === 48 && +code[i + 1] === 2) {
          i += 2;
          bg = colors.match(+code[i], +code[i + 1], +code[i + 2]);
          if (bg === -1) bg = def & 0x1ff;
          i += 2;
          break;
        } else if (c === 38 && +code[i + 1] === 5) {
          i += 2;
          fg = +code[i];
          break;
        } else if (c === 38 && +code[i + 1] === 2) {
          i += 2;
          fg = colors.match(+code[i], +code[i + 1], +code[i + 2]);
          if (fg === -1) fg = (def >> 9) & 0x1ff;
          i += 2;
          break;
        }
        if (c >= 40 && c <= 47) {
          bg = c - 40;
        } else if (c >= 100 && c <= 107) {
          bg = c - 100;
          bg += 8;
        } else if (c === 49) {
          bg = def & 0x1ff;
        } else if (c >= 30 && c <= 37) {
          fg = c - 30;
        } else if (c >= 90 && c <= 97) {
          fg = c - 90;
          fg += 8;
        } else if (c === 39) {
          fg = (def >> 9) & 0x1ff;
        } else if (c === 100) {
          fg = (def >> 9) & 0x1ff;
          bg = def & 0x1ff;
        }
        break;
    }
  }

  return (flags << 18) | (fg << 9) | bg;
}

// codeAttr — verbatim from screen.js (attr int -> SGR string)
function codeAttr(code) {
  var flags = (code >> 18) & 0x1ff
    , fg = (code >> 9) & 0x1ff
    , bg = code & 0x1ff
    , out = '';

  if (flags & 1) out += '1;';
  if (flags & 2) out += '4;';
  if (flags & 4) out += '5;';
  if (flags & 8) out += '7;';
  if (flags & 16) out += '8;';

  if (bg !== 0x1ff) {
    bg = bg; // _reduceColor: tput.colors = 256 for xterm-256color -> no reduction
    if (bg < 16) {
      if (bg < 8) {
        bg += 40;
      } else {
        bg -= 8;
        bg += 100;
      }
      out += bg + ';';
    } else {
      out += '48;5;' + bg + ';';
    }
  }

  if (fg !== 0x1ff) {
    if (fg < 16) {
      if (fg < 8) {
        fg += 30;
      } else {
        fg -= 8;
        fg += 90;
      }
      out += fg + ';';
    } else {
      out += '38;5;' + fg + ';';
    }
  }

  if (out[out.length - 1] === ';') out = out.slice(0, -1);

  return '\x1b[' + out + 'm';
}

// ---------------------------------------------------------------------------
// helpers (helpers.js)
// ---------------------------------------------------------------------------
function stripTags(text) {
  if (!text) return '';
  return text
    .replace(/{(\/?)([\w\-,;!#]*)}/g, '')
    .replace(/\x1b\[[\d;]*m/g, '');
}
function cleanTags(text) {
  return stripTags(text).trim();
}

// ---------------------------------------------------------------------------
// Node (node.js, minimal)
// ---------------------------------------------------------------------------
function Node(options) {
  options = options || {};
  this.type = options.type || 'node';
  this.options = options;
  this.parent = options.parent || null;
  this.children = [];
  this.uid = Node.uid++;
  this.index = -1;
  this.$ = this._ = this.data = {};
  if (options.screen != null) {
    this.screen = options.screen;
  } else if (options.parent) {
    // walk up to find the screen (node.js behavior when parent has a screen)
    this.screen = options.parent;
    while (this.screen && this.screen.type !== 'screen') {
      this.screen = this.screen.parent;
    }
  }
  this.detached = true;
  if (this.parent) {
    this.parent.append(this);
  }
  (options.children || []).forEach(function(el) { this.append(el); }, this);
}

Node.uid = 0;

Node.prototype.insert = function(element, i) {
  element.detach();
  element.parent = this;
  element.screen = this.screen;
  if (i === 0) {
    this.children.unshift(element);
  } else if (i === this.children.length) {
    this.children.push(element);
  } else {
    this.children.splice(i, 0, element);
  }
  (function emit(el) {
    var n = el.detached !== this.detached;
    el.detached = this.detached;
    if (n) el.emit && el.emit('attach');
    el.children.forEach(emit, el);
  }).call(this, element);
  if (this.type === 'screen' && !this.focused) {
    this.focused = element;
  }
};

Node.prototype.append = function(element) {
  this.insert(element, this.children.length);
};

Node.prototype.remove = function(element) {
  if (element.parent !== this) return;
  var i = this.children.indexOf(element);
  if (!~i) return;
  element.parent = null;
  element.screen = null;
  this.children.splice(i, 1);
  (function emit(el) {
    if (el.detached !== true) {
      el.detached = true;
      el.emit && el.emit('detach');
    }
    el.children.forEach(emit, el);
  }).call(this, element);
};

Node.prototype.detach = function() {
  if (this.parent) {
    this.parent.remove(this);
  }
  this.screen = null;
  this.detached = true;
};

Node.prototype.emit = function(type) {
  var args = Array.prototype.slice(arguments, 1);
  (this._handlers[type] || []).slice().forEach(function(fn) { fn.apply(null, args); });
};
Node.prototype.emit = function(type) {
  this._handlers = this._handlers || {};
  (this._handlers[type] || []).slice().forEach(function(fn) { fn.apply(null, args); });
};

// Element._getPos (element.js): from lpos (asserted set by the render above)
Element.prototype._getPos = function() {
  var pos = this.lpos;

  // vtop does not assert; a stale/missing lpos falls back to computed coords.
  if (!pos) return this._getCoords(false);

  if (pos.aleft != null) return pos;

  pos.aleft = pos.xi;
  pos.atop = pos.yi;
  pos.aright = this.screen.cols - pos.xl;
  pos.abottom = this.screen.rows - pos.yl;
  pos.width = pos.xl - pos.xi;
  pos.height = pos.yl - pos.yi;

  return pos;
};

// Screen._getPos returns the screen itself (which has width/height/etc).
Screen.prototype._getPos = function() {
  return this;
};

Node.prototype.on = function(type, handler) {
  this._handlers = this._handlers || {};
  this._handlers[type] = this._handlers[type] || [];
  this._handlers[type].push(handler);
  return this;
};

Node.prototype.removeListener = function(type, handler) {
  if (!this._handlers || !this._handlers[type]) return this;
  var i = this._handlers[type].indexOf(handler);
  if (~i) this._handlers[type].splice(i, 1);
  return this;
};

// ---------------------------------------------------------------------------
// Element (element.js — the full positioning/render subset)
// ---------------------------------------------------------------------------
function Element(options) {
  var self = this;
  options = options || {};

  Node.call(this, options);
  this.type = options.type || 'element';

  options.position = options.position || {
    left: options.left,
    right: options.right,
    top: options.top,
    bottom: options.bottom,
    width: options.width,
    height: options.height
  };

  if (options.position.width === 'shrink'
      || options.position.height === 'shrink') {
    if (options.position.width === 'shrink') {
      delete options.position.width;
    }
    if (options.position.height === 'shrink') {
      delete options.position.height;
    }
    options.shrink = true;
  }

  this.position = options.position;
  this.noOverflow = options.noOverflow;
  this.dockBorders = options.dockBorders;

  this.style = options.style;
  if (!this.style) {
    this.style = {};
    this.style.fg = options.fg;
    this.style.bg = options.bg;
    this.style.bold = options.bold;
    this.style.underline = options.underline;
    this.style.blink = options.blink;
    this.style.inverse = options.inverse;
    this.style.invisible = options.invisible;
    this.style.transparent = options.transparent;
  }

  this.hidden = options.hidden || false;
  this.fixed = options.fixed || false;
  this.align = options.align || 'left';
  this.valign = options.valign || 'top';
  this.wrap = options.wrap !== false;
  this.shrink = options.shrink;
  this.ch = options.ch || ' ';

  if (typeof options.padding === 'number' || !options.padding) {
    options.padding = {
      left: options.padding,
      top: options.padding,
      right: options.padding,
      bottom: options.padding
    };
  }

  this.padding = {
    left: options.padding.left || 0,
    top: options.padding.top || 0,
    right: options.padding.right || 0,
    bottom: options.padding.bottom || 0
  };

  this.border = options.border;
  if (this.border) {
    if (typeof this.border === 'string') {
      this.border = { type: this.border };
    }
    this.border.type = this.border.type || 'bg';
    if (this.border.type === 'ascii') this.border.type = 'line';
    this.border.ch = this.border.ch || ' ';
    this.style.border = this.style.border || this.border.style;
    if (!this.style.border) {
      this.style.border = {};
      this.style.border.fg = this.border.fg;
      this.style.border.bg = this.border.bg;
    }
    if (this.border.left == null) this.border.left = true;
    if (this.border.top == null) this.border.top = true;
    if (this.border.right == null) this.border.right = true;
    if (this.border.bottom == null) this.border.bottom = true;
  }

  this.parseTags = options.parseTags || options.tags;

  this.setContent(options.content || '', true);

  if (options.label) {
    this.setLabel(options.label);
  }
}

Element.prototype.__proto__ = Node.prototype;
Element.prototype.type = 'element';

Element.prototype.sattr = function(style, fg, bg) {
  var bold = style.bold
    , underline = style.underline
    , blink = style.blink
    , inverse = style.inverse
    , invisible = style.invisible;

  if (fg == null && bg == null) {
    fg = style.fg;
    bg = style.bg;
  }

  if (typeof bold === 'function') bold = bold(this);
  if (typeof underline === 'function') underline = underline(this);
  if (typeof blink === 'function') blink = blink(this);
  if (typeof inverse === 'function') inverse = inverse(this);
  if (typeof invisible === 'function') invisible = invisible(this);
  if (typeof fg === 'function') fg = fg(this);
  if (typeof bg === 'function') bg = bg(this);

  return ((invisible ? 16 : 0) << 18)
    | ((inverse ? 8 : 0) << 18)
    | ((blink ? 4 : 0) << 18)
    | ((underline ? 2 : 0) << 18)
    | ((bold ? 1 : 0) << 18)
    | (colors.convert(fg) << 9)
    | colors.convert(bg);
};

Element.prototype.setContent = function(content, noClear, noTags) {
  this.content = content || '';
  this.parseContent(noTags);
  this.emit('set content');
};

Element.prototype.getContent = function() {
  if (!this._clines) return '';
  return this._clines.fake.join('\n');
};

// _parseTags — verbatim from element.js
Element.prototype._parseTags = function(text) {
  if (!this.parseTags) return text;
  if (!/{\/?[\w\-,;!#]*}/.test(text)) return text;

  var screen = this.screen
    , out = ''
    , state
    , bg = []
    , fg = []
    , flag = []
    , cap
    , slash
    , param
    , attr
    , esc;

  for (;;) {
    if (!esc && (cap = /^{escape}/.exec(text))) {
      text = text.substring(cap[0].length);
      esc = true;
      continue;
    }

    if (esc && (cap = /^([\s\S]+?){\/escape}/.exec(text))) {
      text = text.substring(cap[0].length);
      out += cap[1];
      esc = false;
      continue;
    }

    if (esc) {
      out += text;
      break;
    }

    if (cap = /^{(\/?)([\w\-,;!#]*)}/.exec(text)) {
      text = text.substring(cap[0].length);
      slash = cap[1] === '/';
      param = cap[2].replace(/-/g, ' ');

      if (param === 'open') {
        out += '{';
        continue;
      } else if (param === 'close') {
        out += '}';
        continue;
      }

      if (param.slice(-3) === ' bg') state = bg;
      else if (param.slice(-3) === ' fg') state = fg;
      else state = flag;

      if (slash) {
        if (!param) {
          out += programAttr('normal');
          bg.length = 0;
          fg.length = 0;
          flag.length = 0;
        } else {
          attr = programAttr(param, false);
          if (attr == null) {
            out += cap[0];
          } else {
            state.pop();
            if (state.length) {
              out += programAttr(state[state.length - 1]);
            } else {
              out += attr;
            }
          }
        }
      } else {
        if (!param) {
          out += cap[0];
        } else {
          attr = programAttr(param);
          if (attr == null) {
            out += cap[0];
          } else {
            state.push(param);
            out += attr;
          }
        }
      }

      continue;
    }

    if (cap = /^[\s\S]+?(?={\/?[\w\-,;!#]*})/.exec(text)) {
      text = text.substring(cap[0].length);
      out += cap[0];
      continue;
    }

    out += text;
    break;
  }

  return out;
};
Element.prototype._programAttr = programAttr;

Element.prototype._parseAttr = function(lines) {
  var dattr = this.sattr(this.style)
    , attr = dattr
    , attrs = []
    , line
    , i
    , j
    , c;

  // vtop never leaves attr === dattr at line 0 (lines[0].attr check in real
  // blessed compares a number to this dattr only when lines have .attr set;
  // our plain strings never do).

  for (j = 0; j < lines.length; j++) {
    line = lines[j];
    attrs[j] = attr;
    for (i = 0; i < line.length; i++) {
      if (line[i] === '\x1b') {
        if (c = /^\x1b\[[\d;]*m/.exec(line.substring(i))) {
          attr = attrCode(c[0], attr, dattr);
          i += c[0].length - 1;
        }
      }
    }
  }

  return attrs;
};

// _align — verbatim
Element.prototype._align = function(line, width, align) {
  if (!align) return line;

  var cline = line.replace(/\x1b\[[\d;]*m/g, '')
    , len = cline.length
    , s = width - len;

  if (this.shrink) {
    s = 0;
  }

  if (len === 0) return line;
  if (s < 0) return line;

  if (align === 'center') {
    s = Array(((s / 2) | 0) + 1).join(' ');
    return s + line + s;
  } else if (align === 'right') {
    s = Array(s + 1).join(' ');
    return s + line;
  } else if (this.parseTags && ~line.indexOf('{|}')) {
    var parts = line.split('{|}')
      , cparts = cline.split('{|}');
    s = Math.max(width - cparts[0].length - cparts[1].length, 0);
    s = Array(s + 1).join(' ');
    return parts[0] + s + parts[1];
  }

  return line;
};

// _wrapContent — verbatim (this screen is never fullUnicode)
Element.prototype._wrapContent = function(content, width) {
  var tags = this.parseTags
    , state = this.align
    , wrap = this.wrap
    , rtof = []
    , ftor = []
    , out = []
    , no = 0
    , line
    , align
    , cap
    , total
    , i
    , part
    , j
    , lines
    , rest;

  lines = content.split('\n');

  if (!content) {
    out.push(content);
    out.rtof = [0];
    out.ftor = [[0]];
    out.fake = lines;
    out.real = out;
    out.mwidth = 0;
    return out;
  }

  if (this.scrollbar) var margin = 1;
  if (this.type === 'textarea') margin = (margin || 0) + 1;
  if (width > (margin || 0)) width -= (margin || 0);

main:
  for (; no < lines.length; no++) {
    line = lines[no];
    align = state;

    ftor.push([]);

    if (tags) {
      if (cap = /^{(left|center|right)}/.exec(line)) {
        line = line.substring(cap[0].length);
        align = state = cap[1] !== 'left'
          ? cap[1]
          : null;
      }
      if (cap = /{\/(left|center|right)}$/.exec(line)) {
        line = line.slice(0, -cap[0].length);
        state = this.align;
      }
    }

    while (line.length > width) {
      for (i = 0, total = 0; i < line.length; i++) {
        while (line[i] === '\x1b') {
          while (line[i] && line[i++] !== 'm');
        }
        if (!line[i]) break;
        if (++total === width) {
          i++;
          if (!wrap) {
            rest = line.substring(i).match(/\x1b\[[^m]*m/g);
            rest = rest ? rest.join('') : '';
            out.push(this._align(line.substring(0, i) + rest, width, align));
            ftor[no].push(out.length - 1);
            rtof.push(no);
            continue main;
          }
          if (i !== line.length) {
            j = i;
            while (j > i - 10 && j > 0 && line[--j] !== ' ');
            if (line[j] === ' ') i = j + 1;
          }
          break;
        }
      }

      part = line.substring(0, i);
      line = line.substring(i);

      out.push(this._align(part, width, align));
      ftor[no].push(out.length - 1);
      rtof.push(no);

      if (line === '') continue main;

      if (/^(?:\x1b[\[\d;]*m)+$/.test(line)) {
        out[out.length - 1] += line;
        continue main;
      }
    }

    out.push(this._align(line, width, align));
    ftor[no].push(out.length - 1);
    rtof.push(no);
  }

  out.rtof = rtof;
  out.ftor = ftor;
  out.fake = lines;
  out.real = out;

  out.mwidth = out.reduce(function(current, line) {
    line = line.replace(/\x1b\[[\d;]*m/g, '');
    return line.length > current
      ? line.length
      : current;
  }, 0);

  return out;
};

Element.prototype.parseContent = function(noTags) {
  if (this.detached) return false;

  var width = this.width - this.iwidth;
  if (this._clines == null
      || this._clines.width !== width
      || this._clines.content !== this.content) {
    var content = this.content;

    content = content
      .replace(/[\x00-\x08\x0b-\x0c\x0e-\x1a\x1c-\x1f\x7f]/g, '')
      .replace(/\x1b(?!\[[\d;]*m)/g, '')
      .replace(/\r\n|\r/g, '\n')
      .replace(/\t/g, this.screen.tabc);

    if (!noTags) {
      content = this._parseTags(content);
    }

    this._clines = this._wrapContent(content, width);
    this._clines.width = width;
    this._clines.content = this.content;
    this._clines.attr = this._parseAttr(this._clines);
    this._clines.ci = [];
    this._clines.reduce(function(total, line) {
      this._clines.ci.push(total);
      return total + line.length + 1;
    }.bind(this), 0);

    this._pcontent = this._clines.join('\n');
    this.emit('parsed content');

    return true;
  }

  this._clines.attr = this._parseAttr(this._clines) || this._clines.attr;

  return false;
};

// Position getters — verbatim from element.js
Element.prototype._getWidth = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , width = this.position.width
    , left
    , expr;

  if (typeof width === 'string') {
    if (width === 'half') width = '50%';
    expr = width.split(/(?=\+|-)/);
    width = expr[0];
    width = +width.slice(0, -1) / 100;
    width = parent.width * width | 0;
    width += +(expr[1] || 0);
    return width;
  }

  if (width == null) {
    left = this.position.left || 0;
    if (typeof left === 'string') {
      if (left === 'center') left = '50%';
      expr = left.split(/(?=\+|-)/);
      left = expr[0];
      left = +left.slice(0, -1) / 100;
      left = parent.width * left | 0;
      left += +(expr[1] || 0);
    }
    width = parent.width - (this.position.right || 0) - left;
    if (this.screen.autoPadding) {
      if ((this.position.left != null || this.position.right == null)
          && this.position.left !== 'center') {
        width -= this.parent.ileft;
      }
      width -= this.parent.iright;
    }
  }

  return width;
};

Element.prototype._getHeight = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , height = this.position.height
    , top
    , expr;

  if (typeof height === 'string') {
    if (height === 'half') height = '50%';
    expr = height.split(/(?=\+|-)/);
    height = expr[0];
    height = +height.slice(0, -1) / 100;
    height = parent.height * height | 0;
    height += +(expr[1] || 0);
    return height;
  }

  if (height == null) {
    top = this.position.top || 0;
    if (typeof top === 'string') {
      if (top === 'center') top = '50%';
      expr = top.split(/(?=\+|-)/);
      top = expr[0];
      top = +top.slice(0, -1) / 100;
      top = parent.height * top | 0;
      top += +(expr[1] || 0);
    }
    height = parent.height - (this.position.bottom || 0) - top;
    if (this.screen.autoPadding) {
      if ((this.position.top != null
          || this.position.bottom == null)
          && this.position.top !== 'center') {
        height -= this.parent.itop;
      }
      height -= this.parent.ibottom;
    }
  }

  return height;
};

Element.prototype._getLeft = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , left = this.position.left || 0
    , expr;

  if (typeof left === 'string') {
    if (left === 'center') left = '50%';
    expr = left.split(/(?=\+|-)/);
    left = expr[0];
    left = +left.slice(0, -1) / 100;
    left = parent.width * left | 0;
    left += +(expr[1] || 0);
    if (this.position.left === 'center') {
      left -= this._getWidth(get) / 2 | 0;
    }
  }

  if (this.position.left == null && this.position.right != null) {
    return this.screen.cols - this._getWidth(get) - this._getRight(get);
  }

  if (this.screen.autoPadding) {
    if ((this.position.left != null
        || this.position.right == null)
        && this.position.left !== 'center') {
      left += this.parent.ileft;
    }
  }

  return (parent.aleft || 0) + left;
};

Element.prototype._getRight = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , right;

  if (this.position.right == null && this.position.left != null) {
    right = this.screen.cols - (this._getLeft(get) + this._getWidth(get));
    if (this.screen.autoPadding) {
      right += this.parent.iright;
    }
    return right;
  }

  right = (parent.aright || 0) + (this.position.right || 0);

  if (this.screen.autoPadding) {
    right += this.parent.iright;
  }

  return right;
};

Element.prototype._getTop = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , top = this.position.top || 0
    , expr;

  if (typeof top === 'string') {
    if (top === 'center') top = '50%';
    expr = top.split(/(?=\+|-)/);
    top = expr[0];
    top = +top.slice(0, -1) / 100;
    top = parent.height * top | 0;
    top += +(expr[1] || 0);
    if (this.position.top === 'center') {
      top -= this._getHeight(get) / 2 | 0;
    }
  }

  if (this.position.top == null && this.position.bottom != null) {
    return this.screen.rows - this._getHeight(get) - this._getBottom(get);
  }

  if (this.screen.autoPadding) {
    if ((this.position.top != null
        || this.position.bottom == null)
        && this.position.top !== 'center') {
      top += this.parent.itop;
    }
  }

  return (parent.atop || 0) + top;
};

Element.prototype._getBottom = function(get) {
  var parent = get ? this.parent._getPos() : this.parent
    , bottom;

  if (this.position.bottom == null && this.position.top != null) {
    bottom = this.screen.rows - (this._getTop(get) + this._getHeight(get));
    if (this.screen.autoPadding) {
      bottom += this.parent.ibottom;
    }
    return bottom;
  }

  bottom = (parent.abottom || 0) + (this.position.bottom || 0);

  if (this.screen.autoPadding) {
    bottom += this.parent.ibottom;
  }

  return bottom;
};

['ileft', 'itop', 'iright', 'ibottom', 'iwidth', 'iheight'].forEach(function(key) {});

Element.prototype.__defineGetter__('ileft', function() {
  return (this.border ? 1 : 0) + this.padding.left;
});
Element.prototype.__defineGetter__('itop', function() {
  return (this.border ? 1 : 0) + this.padding.top;
});
Element.prototype.__defineGetter__('iright', function() {
  return (this.border ? 1 : 0) + this.padding.right;
});
Element.prototype.__defineGetter__('ibottom', function() {
  return (this.border ? 1 : 0) + this.padding.bottom;
});
Element.prototype.__defineGetter__('iwidth', function() {
  return (this.border ? 2 : 0) + this.padding.left + this.padding.right;
});
Element.prototype.__defineGetter__('iheight', function() {
  return (this.border ? 2 : 0) + this.padding.top + this.padding.bottom;
});
Element.prototype.__defineGetter__('tpadding', function() {
  return this.padding.left + this.padding.top
    + this.padding.right + this.padding.bottom;
});

Element.prototype.__defineGetter__('aleft', function() {
  return this._getLeft(false);
});
Element.prototype.__defineGetter__('aright', function() {
  return this._getRight(false);
});
Element.prototype.__defineGetter__('atop', function() {
  return this._getTop(false);
});
Element.prototype.__defineGetter__('abottom', function() {
  return this._getBottom(false);
});
Element.prototype.__defineGetter__('rleft', function() {
  return this.aleft - this.parent.aleft;
});
Element.prototype.__defineGetter__('rright', function() {
  return this.aright - this.parent.aright;
});
Element.prototype.__defineGetter__('rtop', function() {
  return this.atop - this.parent.atop;
});
Element.prototype.__defineGetter__('rbottom', function() {
  return this.abottom - this.parent.abottom;
});
Element.prototype.__defineGetter__('left', function() {
  return this.rleft;
});
Element.prototype.__defineGetter__('right', function() {
  return this.rright;
});
Element.prototype.__defineGetter__('top', function() {
  return this.rtop;
});
Element.prototype.__defineGetter__('bottom', function() {
  return this.rbottom;
});
Element.prototype.__defineGetter__('width', function() {
  return this._getWidth(false);
});
Element.prototype.__defineGetter__('height', function() {
  return this._getHeight(false);
});

Element.prototype.__defineSetter__('width', function(val) {
  if (this.position.width === val) return;
  if (/^\d+$/.test(val)) val = +val;
  this.clearPos();
  return this.position.width = val;
});
Element.prototype.__defineSetter__('height', function(val) {
  if (this.position.height === val) return;
  if (/^\d+$/.test(val)) val = +val;
  this.clearPos();
  return this.position.height = val;
});
Element.prototype.__defineSetter__('rleft', function(val) {
  if (this.position.left === val) return;
  if (/^\d+$/.test(val)) val = +val;
  this.clearPos();
  return this.position.left = val;
});
Element.prototype.__defineSetter__('rright', function(val) {
  if (this.position.right === val) return;
  this.clearPos();
  return this.position.right = val;
});
Element.prototype.__defineSetter__('rtop', function(val) {
  if (this.position.top === val) return;
  if (/^\d+$/.test(val)) val = +val;
  this.clearPos();
  return this.position.top = val;
});
Element.prototype.__defineSetter__('rbottom', function(val) {
  if (this.position.bottom === val) return;
  this.clearPos();
  return this.position.bottom = val;
});
Element.prototype.__defineSetter__('aleft', function(val) {
  var expr;
  if (typeof val === 'string') {
    if (val === 'center') {
      val = this.screen.width / 2 | 0;
      val -= this.width / 2 | 0;
    } else {
      expr = val.split(/(?=\+|-)/);
      val = expr[0];
      val = +val.slice(0, -1) / 100;
      val = this.screen.width * val | 0;
      val += +(expr[1] || 0);
    }
  }
  val -= this.parent.aleft;
  if (this.position.left === val) return;
  this.clearPos();
  return this.position.left = val;
});

Element.prototype.clearPos = function(get, override) {
  if (this.detached) return;
  var lpos = this._getCoords(get);
  if (!lpos) return;
  this.screen.clearRegion(
    lpos.xi, lpos.xl,
    lpos.yi, lpos.yl,
    override);
};

Element.prototype.focus = function() {
  return this.screen.focused = this;
};

// setLabel — verbatim (label box: rleft = 2 - ileft, rtop = childBase - itop)
Element.prototype.setLabel = function(options) {
  var self = this;

  if (typeof options === 'string') {
    options = { text: options };
  }

  this._label = new Box({
    screen: this.screen,
    parent: this,
    content: options.text,
    top: -this.itop,
    tags: this.parseTags,
    shrink: true,
    style: this.style.label
  });

  if (options.side !== 'right') {
    this._label.rleft = 2 - this.ileft;
  } else {
    this._label.rright = 2 - this.iright;
  }

  this._label._isLabel = true;

  var reposition = function() {
    self._label.rtop = (self.childBase || 0) - self.itop;
    self.screen.render();
  };

  this.on('scroll', function() { reposition(); });
};

// _getCoords — verbatim
Element.prototype._getCoords = function(get, noscroll) {
  if (this.hidden) return;

  var xi = this._getLeft(get)
    , xl = xi + this._getWidth(get)
    , yi = this._getTop(get)
    , yl = yi + this._getHeight(get)
    , base = this.childBase || 0
    , el = this
    , fixed = this.fixed
    , coords
    , v
    , noleft
    , noright
    , notop
    , nobot
    , ppos
    , b;

  if (this.shrink) {
    coords = this._getShrink(xi, xl, yi, yl, get);
    xi = coords.xi, xl = coords.xl;
    yi = coords.yi, yl = coords.yl;
  }

  while (el = el.parent) {
    if (el.scrollable) {
      if (fixed) {
        fixed = false;
        continue;
      }
      break;
    }
  }

  var thisparent = el;
  if (el && !noscroll) {
    ppos = thisparent.lpos;

    if (!ppos) return;

    yi -= ppos.base;
    yl -= ppos.base;

    b = thisparent.border ? 1 : 0;

    if (this._isLabel) {
      b = 0;
    }

    if (yi < ppos.yi + b) {
      if (yl - 1 < ppos.yi + b) {
        return;
      } else {
        notop = true;
        v = ppos.yi - yi;
        if (this.border) v--;
        if (thisparent.border) v++;
        base += v;
        yi += v;
      }
    } else if (yl > ppos.yl - b) {
      if (yi > ppos.yl - 1 - b) {
        return;
      } else {
        nobot = true;
        v = yl - ppos.yl;
        if (this.border) v--;
        if (thisparent.border) v++;
        yl -= v;
      }
    }

    if (yi >= yl) return;

    if (xi < el.lpos.xi) {
      xi = el.lpos.xi;
      noleft = true;
      if (this.border) xi--;
      if (thisparent.border) xi++;
    }
    if (xl > el.lpos.xl) {
      xl = el.lpos.xl;
      noright = true;
      if (this.border) xl++;
      if (thisparent.border) xl--;
    }
    if (xi >= xl) return;
  }

  if (this.noOverflow && this.parent.lpos) {
    if (xi < this.parent.lpos.xi + this.parent.ileft) {
      xi = this.parent.lpos.xi + this.parent.ileft;
    }
    if (xl > this.parent.lpos.xl - this.parent.iright) {
      xl = this.parent.lpos.xl - this.parent.iright;
    }
    if (yi < this.parent.lpos.yi + this.parent.itop) {
      yi = this.parent.lpos.yi + this.parent.itop;
    }
    if (yl > this.parent.lpos.yl - this.parent.ibottom) {
      yl = this.parent.lpos.yl - this.parent.ibottom;
    }
  }

  return {
    xi: xi,
    xl: xl,
    yi: yi,
    yl: yl,
    base: base,
    noleft: noleft,
    noright: noright,
    notop: notop,
    nobot: nobot,
    renders: this.screen.renders
  };
};

Element.prototype.render = function() {
  this.parseContent();

  var coords = this._getCoords(true);
  if (!coords) {
    delete this.lpos;
    return;
  }

  if (coords.xl - coords.xi <= 0) {
    coords.xl = Math.max(coords.xl, coords.xi);
    return;
  }

  if (coords.yl - coords.yi <= 0) {
    coords.yl = Math.max(coords.yl, coords.yi);
    return;
  }

  var lines = this.screen.lines
    , xi = coords.xi
    , xl = coords.xl
    , yi = coords.yi
    , yl = coords.yl
    , x
    , y
    , cell
    , attr
    , ch
    , content = this._pcontent
    , ci = this._clines.ci[coords.base]
    , battr
    , dattr
    , c
    , visible
    , i
    , bch = this.ch;

  if (coords.base >= this._clines.ci.length) {
    ci = this._pcontent.length;
  }

  this.lpos = coords;

  dattr = this.sattr(this.style);
  attr = dattr;

  if (ci > 0) {
    attr = this._clines.attr[Math.min(coords.base, this._clines.length - 1)];
  }

  if (this.border) xi++, xl--, yi++, yl--;

  if (this.tpadding || (this.valign && this.valign !== 'top')) {
    this.screen.fillRegion(dattr, bch, xi, xl, yi, yl);
  }

  if (this.tpadding) {
    xi += this.padding.left, xl -= this.padding.right;
    yi += this.padding.top, yl -= this.padding.bottom;
  }

  if (this.valign === 'middle' || this.valign === 'bottom') {
    visible = yl - yi;
    if (this._clines.length < visible) {
      if (this.valign === 'middle') {
        visible = visible / 2 | 0;
        visible -= this._clines.length / 2 | 0;
      } else if (this.valign === 'bottom') {
        visible -= this._clines.length;
      }
      ci -= visible * (xl - xi);
    }
  }

  for (y = yi; y < yl; y++) {
    if (!lines[y]) {
      if (y >= this.screen.height || yl < this.ibottom) {
        break;
      } else {
        continue;
      }
    }
    for (x = xi; x < xl; x++) {
      cell = lines[y][x];
      if (!cell) {
        if (x >= this.screen.width || xl < this.iright) {
          break;
        } else {
          continue;
        }
      }

      ch = content[ci++] || bch;

      while (ch === '\x1b') {
        if (c = /^\x1b\[[\d;]*m/.exec(content.substring(ci - 1))) {
          ci += c[0].length - 1;
          attr = attrCode(c[0], attr, dattr);
          if (this.parent._isList && this.parent.interactive
              && this.parent.items[this.parent.selected] === this
              && this.parent.options.invertSelected !== false) {
            attr = (attr & ~(0x1ff << 9)) | (dattr & (0x1ff << 9));
          }
          ch = content[ci] || bch;
          ci++;
        } else {
          break;
        }
      }

      if (ch === '\t') ch = bch;
      if (ch === '\n') {
        if (x === xi && y !== yi && content[ci - 2] !== '\n') {
          x--;
          continue;
        }
        ch = bch;
        for (; x < xl; x++) {
          cell = lines[y][x];
          if (!cell) break;
          if (attr !== cell[0] || ch !== cell[1]) {
            lines[y][x][0] = attr;
            lines[y][x][1] = ch;
            lines[y].dirty = true;
          }
        }
        continue;
      }

      if (attr !== cell[0] || ch !== cell[1]) {
        lines[y][x][0] = attr;
        lines[y][x][1] = ch;
        lines[y].dirty = true;
      }
    }
  }

  if (this.border) xi--, xl++, yi--, yl++;

  if (this.tpadding) {
    xi -= this.padding.left, xl += this.padding.right;
    yi -= this.padding.top, yl += this.padding.bottom;
  }

  if (this.border) {
    battr = this.sattr(this.style.border);
    y = yi;
    if (coords.notop) y = -1;
    for (x = xi; x < xl; x++) {
      if (!lines[y]) break;
      if (coords.noleft && x === xi) continue;
      if (coords.noright && x === xl - 1) continue;
      cell = lines[y][x];
      if (!cell) continue;
      if (this.border.type === 'line') {
        if (x === xi) {
          ch = '\u250c';
          if (!this.border.left) {
            if (this.border.top) {
              ch = '\u2500';
            } else {
              continue;
            }
          } else {
            if (!this.border.top) {
              ch = '\u2502';
            }
          }
        } else if (x === xl - 1) {
          ch = '\u2510';
          if (!this.border.right) {
            if (this.border.top) {
              ch = '\u2500';
            } else {
              continue;
            }
          } else {
            if (!this.border.top) {
              ch = '\u2502';
            }
          }
        } else {
          ch = '\u2500';
        }
      } else if (this.border.type === 'bg') {
        ch = this.border.ch;
      }
      if (!this.border.top && x !== xi && x !== xl - 1) {
        ch = ' ';
        if (dattr !== cell[0] || ch !== cell[1]) {
          lines[y][x][0] = dattr;
          lines[y][x][1] = ch;
          lines[y].dirty = true;
          continue;
        }
      }
      if (battr !== cell[0] || ch !== cell[1]) {
        lines[y][x][0] = battr;
        lines[y][x][1] = ch;
        lines[y].dirty = true;
      }
    }
    y = yi + 1;
    for (; y < yl - 1; y++) {
      if (!lines[y]) continue;
      cell = lines[y][xi];
      if (cell) {
        if (this.border.left) {
          if (this.border.type === 'line') {
            ch = '\u2502';
          } else if (this.border.type === 'bg') {
            ch = this.border.ch;
          }
          if (battr !== cell[0] || ch !== cell[1]) {
            lines[y][xi][0] = battr;
            lines[y][xi][1] = ch;
            lines[y].dirty = true;
          }
        } else {
          if (dattr !== cell[0] || ' ' !== cell[1]) {
            lines[y][xi][0] = dattr;
            lines[y][xi][1] = ' ';
            lines[y].dirty = true;
          }
        }
      }
      cell = lines[y][xl - 1];
      if (cell) {
        if (this.border.right) {
          if (this.border.type === 'line') {
            ch = '\u2502';
          } else if (this.border.type === 'bg') {
            ch = this.border.ch;
          }
          if (battr !== cell[0] || ch !== cell[1]) {
            lines[y][xl - 1][0] = battr;
            lines[y][xl - 1][1] = ch;
            lines[y].dirty = true;
          }
        } else {
          if (dattr !== cell[0] || ' ' !== cell[1]) {
            lines[y][xl - 1][0] = dattr;
            lines[y][xl - 1][1] = ' ';
            lines[y].dirty = true;
          }
        }
      }
    }
    y = yl - 1;
    if (coords.nobot) y = -1;
    for (x = xi; x < xl; x++) {
      if (!lines[y]) break;
      if (coords.noleft && x === xi) continue;
      if (coords.noright && x === xl - 1) continue;
      cell = lines[y][x];
      if (!cell) continue;
      if (this.border.type === 'line') {
        if (x === xi) {
          ch = '\u2514';
          if (!this.border.left) {
            if (this.border.bottom) {
              ch = '\u2500';
            } else {
              continue;
            }
          } else {
            if (!this.border.bottom) {
              ch = '\u2502';
            }
          }
        } else if (x === xl - 1) {
          ch = '\u2518';
          if (!this.border.right) {
            if (this.border.bottom) {
              ch = '\u2500';
            } else {
              continue;
            }
          } else {
            if (!this.border.bottom) {
              ch = '\u2502';
            }
          }
        } else {
          ch = '\u2500';
        }
      } else if (this.border.type === 'bg') {
        ch = this.border.ch;
      }
      if (!this.border.bottom && x !== xi && x !== xl - 1) {
        ch = ' ';
        if (dattr !== cell[0] || ch !== cell[1]) {
          lines[y][x][0] = dattr;
          lines[y][x][1] = ch;
          lines[y].dirty = true;
          continue;
        }
      }
      if (battr !== cell[0] || ch !== cell[1]) {
        lines[y][x][0] = battr;
        lines[y][x][1] = ch;
        lines[y].dirty = true;
      }
    }
  }

  this.children.forEach(function(el) {
    el.render();
  });

  return coords;
};

// _getShrink etc. (unused for vtop's boxes but present for completeness)
Element.prototype._getShrinkBox = function(xi, xl, yi, yl, get) {
  if (!this.children.length) {
    return { xi: xi, xl: xi + 1, yi: yi, yl: yi + 1 };
  }
  var mxi = xi, mxl = xi + 1, myi = yi, myl = yi + 1;
  var i, el, ret;
  for (i = 0; i < this.children.length; i++) {
    el = this.children[i];
    ret = el._getCoords(get);
    if (!ret) continue;
    if (ret.xi < mxi) mxi = ret.xi;
    if (ret.xl > mxl) mxl = ret.xl;
    if (ret.yi < myi) myi = ret.yi;
    if (ret.yl > myl) myl = ret.yl;
  }
  return { xi: xi, xl: xl, yi: yi, yl: yl, mxi: mxi, mxl: mxl, myi: myi, myl: myl };
};

Element.prototype._getShrinkContent = function(xi, xl, yi, yl) {
  var h = this._clines.length
    , w = this._clines.mwidth || 1;

  if (this.position.width == null
      && (this.position.left == null
      || this.position.right == null)) {
    if (this.position.left == null && this.position.right != null) {
      xi = xl - w - this.iwidth;
    } else {
      xl = xi + w + this.iwidth;
    }
  }

  if (this.position.height == null
      && (this.position.top == null
      || this.position.bottom == null)
      && (!this.scrollable || this._isList)) {
    if (this.position.top == null && this.position.bottom != null) {
      yi = yl - h - this.iheight;
    } else {
      yl = yi + h + this.iheight;
    }
  }

  return { xi: xi, xl: xl, yi: yi, yl: yl };
};

Element.prototype._getShrink = function(xi, xl, yi, yl, get) {
  var shrinkBox = this._getShrinkBox(xi, xl, yi, yl, get)
    , shrinkContent = this._getShrinkContent(xi, xl, yi, yl, get)
    , xll = xl
    , yll = yl;

  if (shrinkBox.xl - shrinkBox.xi > shrinkContent.xl - shrinkContent.xi) {
    xi = shrinkBox.xi;
    xl = shrinkBox.xl;
  } else {
    xi = shrinkContent.xi;
    xl = shrinkContent.xl;
  }

  if (shrinkBox.yl - shrinkBox.yi > shrinkContent.yl - shrinkContent.yi) {
    yi = shrinkBox.yi;
    yl = shrinkBox.yl;
  } else {
    yi = shrinkContent.yi;
    yl = shrinkContent.yl;
  }

  if (xl < xll && this.position.left === 'center') {
    xll = (xll - xl) / 2 | 0;
    xi += xll;
    xl += xll;
  }

  if (yl < yll && this.position.top === 'center') {
    yll = (yll - yl) / 2 | 0;
    yi += yll;
    yl += yll;
  }

  return { xi: xi, xl: xl, yi: yi, yl: yl };
};

// ---------------------------------------------------------------------------
// Box (box.js — Element with type 'box')
// ---------------------------------------------------------------------------
function Box(options) {
  if (!(this instanceof Node)) {
    return new Box(options);
  }
  options = options || {};
  Element.call(this, options);
  this.type = 'box';
}

Box.prototype.__proto__ = Element.prototype;
Box.prototype.type = 'box';

// ---------------------------------------------------------------------------
// Text (text.js — Box with shrink: true)
// ---------------------------------------------------------------------------
function Text(options) {
  if (!(this instanceof Node)) {
    return new Text(options);
  }
  options = options || {};
  options.shrink = true;
  Element.call(this, options);
  this.type = 'text';
}
Text.prototype.__proto__ = Element.prototype;
Text.prototype.type = 'text';

// ---------------------------------------------------------------------------
// List (list.js + minimal ScrollableBox pieces)
// ---------------------------------------------------------------------------
function List(options) {
  var self = this;

  if (!(this instanceof Node)) {
    return new List(options);
  }

  options = options || {};
  options.ignoreKeys = true;
  options.scrollable = true;
  Box.call(this, options);
  this.type = 'list';

  this.scrollable = true;
  this.childOffset = 0;
  this.childBase = 0;
  this.baseLimit = options.baseLimit || Infinity;
  this.alwaysScroll = options.alwaysScroll;
  this.scrollbar = options.scrollbar;

  this.value = '';
  this.items = [];
  this.ritems = [];
  this.selected = 0;
  this._isList = true;

  this.interactive = options.interactive !== false;
  this.mouse = options.mouse || false;

  if (options.items) {
    this.ritems = options.items;
    options.items.forEach(function(item) { self.add(item); });
  }

  this.select(0);
}

List.prototype.__proto__ = Box.prototype;
List.prototype.type = 'list';

List.prototype.createItem = function(content) {
  var self = this
    , previous = this.items.length
    , last = this.items.length === 0
    , item;

  item = new Box({
    screen: this.screen,
    content: content,
    align: this.align || 'left',
    top: 0,
    left: 0,
    right: (this.scrollbar ? 1 : 0),
    tags: this.parseTags,
    height: 1,
    style: {
      bg: function() {
        var attr = self.items[self.selected] === item && self.interactive
          ? self.style.selected.bg
          : self.style.item.bg;
        if (typeof attr === 'function') attr = attr(item);
        return attr;
      },
      fg: function() {
        var attr = self.items[self.selected] === item && self.interactive
          ? self.style.selected.fg
          : self.style.item.fg;
        if (typeof attr === 'function') attr = attr(item);
        return attr;
      },
      bold: function() {
        var attr = self.items[self.selected] === item && self.interactive
          ? self.style.selected.bold
          : self.style.item.bold;
        if (typeof attr === 'function') attr = attr(item);
        return attr;
      }
    },
    autoFocus: false
  });

  return item;
};

List.prototype.add =
List.prototype.addItem =
List.prototype.appendItem = function(content) {
  content = typeof content === 'string' ? content : content.getContent();

  var item = this.createItem(content);
  item.position.top = this.items.length;

  this.ritems.push(content);
  this.items.push(item);
  this.append(item);

  if (this.items.length === 1) {
    this.select(0);
  }

  return item;
};

List.prototype.removeItem = function(child) {
  var i = this.getItemIndex(child);
  if (~i && this.items[i]) {
    child = this.items.splice(i, 1)[0];
    this.ritems.splice(i, 1);
    this.remove(child);
    for (var j = i; j < this.items.length; j++) {
      this.items[j].position.top--;
    }
    if (i === this.selected) {
      this.select(i - 1);
    }
  }
  return child;
};

List.prototype.getItem = function(child) {
  return this.items[this.getItemIndex(child)];
};

List.prototype.getItemIndex = function(child) {
  if (typeof child === 'number') return child;
  return this.items.indexOf(child);
};

List.prototype.setItem = function(child, content) {
  content = typeof content === 'string' ? content : content.getContent();
  var i = this.getItemIndex(child);
  if (!~i) return;
  this.items[i].setContent(content);
  this.ritems[i] = content;
};

List.prototype.setItems = function(items) {
  var original = this.items.slice()
    , selected = this.selected
    , sel = this.ritems[this.selected]
    , i = 0;

  items = items.slice();

  this.select(0);

  for (; i < items.length; i++) {
    if (this.items[i]) {
      this.items[i].setContent(items[i]);
    } else {
      this.add(items[i]);
    }
  }

  for (; i < original.length; i++) {
    this.remove(original[i]);
  }

  this.ritems = items;

  sel = items.indexOf(sel);
  if (~sel) {
    this.select(sel);
  } else if (items.length === original.length) {
    this.select(selected);
  } else {
    this.select(Math.min(selected, items.length - 1));
  }
};

List.prototype.select = function(index) {
  if (!this.interactive) {
    return;
  }

  if (!this.items.length) {
    this.selected = 0;
    this.value = '';
    this.scrollTo(0);
    return;
  }

  if (typeof index === 'object') {
    index = this.items.indexOf(index);
  }

  if (index < 0) {
    index = 0;
  } else if (index >= this.items.length) {
    index = this.items.length - 1;
  }

  if (this.selected === index && this._listInitialized) return;
  this._listInitialized = true;

  this.selected = index;
  this.value = cleanTags(this.ritems[this.selected]);
  if (!this.parent) return;
  this.scrollTo(this.selected);
};

List.prototype.move = function(offset) {
  this.select(this.selected + offset);
};
List.prototype.up = function(offset) {
  this.move(-(offset || 1));
};
List.prototype.down = function(offset) {
  this.move(offset || 1);
};

List.prototype._scrollBottom = function() {
  if (!this.scrollable) return 0;
  if (this._isList) {
    return this.items ? this.items.length : 0;
  }
  return 0;
};

// scroll + scrollTo — verbatim from scrollablebox.js
List.prototype.scroll = function(offset, always) {
  if (!this.scrollable) return;
  if (this.detached) return;

  var visible = this.height - this.iheight
    , base = this.childBase
    , d
    , max
    , emax;

  if (this._clines == null) {
    // Real blessed's ScrollableBox ctor + `scrollable` Element path guarantee
    // _clines exists (parseContent runs in the ctor via `new Box` → options
    // parse? no: element ctor setContent + ScrollableBox ctor parseContent
    // happens in _recalculateIndex at construction). In 0.1.81 the ctor calls
    // parseContent indirectly (node 'prerender'/select → scrollTo → scroll →
    // parseContent BEFORE touching _clines). Mirror fix: lazily parse.
    this.parseContent();
  }

  if (this.alwaysScroll || always) {
    this.childOffset = offset > 0
      ? visible - 1 + offset
      : offset;
  } else {
    this.childOffset = (this.childOffset || 0) + offset;
  }

  if (this.childOffset > visible - 1) {
    d = this.childOffset - (visible - 1);
    this.childOffset -= d;
    this.childBase = (this.childBase || 0) + d;
  } else if (this.childOffset < 0) {
    d = this.childOffset;
    this.childOffset += -d;
    this.childBase = (this.childBase || 0) + d;
  }

  if (this.childBase < 0) {
    this.childBase = 0;
  }

  if (this.childBase === base) {
    return;
  }

  max = this._clines.length - (this.height - this.iheight);
  if (max < 0) max = 0;
  emax = this._scrollBottom() - (this.height - this.iheight);
  if (emax < 0) emax = 0;

  this.childBase = Math.min(this.childBase, Math.max(emax, max));

  if (this.childBase < 0) {
    this.childBase = 0;
  }

  this.emit('scroll');
};

List.prototype.setScroll =
List.prototype.scrollTo = function(offset, always) {
  this.scroll(0);
  return this.scroll(offset - (this.childBase + this.childOffset), always);
};

List.prototype.getScroll = function() {
  return this.childBase + this.childOffset;
};

List.prototype.getScrollHeight = function() {
  return Math.max(this._clines.length, this._scrollBottom()) - (this.height - this.iheight);
};

// ---------------------------------------------------------------------------
// Screen (screen.js — alloc/render/draw subset; no CSR/cuf/BCE/ACS: C9)
// ---------------------------------------------------------------------------
function Screen(options) {
  var self = this;

  if (!(this instanceof Node)) {
    return new Screen(options);
  }

  Screen.bind(this);

  options = options || {};

  this.screen = this; // Screen.bind does this in the oracle (screen.js:47-49)

  this.options = options;
  this.input = options.input;
  this.output = options.output;
  this.program = options.program;

  if (!this.program) {
    // Our minimal program shim (vtop-mirror path): collects writes.
    this.program = {
      cols: options.cols || options.width,
      rows: options.rows || options.height,
      buffers: options.buffers || [],
      _write: function(s) { this.buffers.push(s); },
      write: function(s) { this.buffers.push(s); },
      flush: function() {},
      tput: {
        colors: options.colors || 256,
        cup: cup,
        el: function() { return '\x1b[K'; },
        smacs: function() { return ''; },
        rmacs: function() { return ''; }
      },
      cursorHidden: options.cursorHidden === true
    };
  }

  this.tput = this.program.tput;

  this.autoPadding = options.autoPadding !== false;
  this.tabc = Array((options.tabSize || 4) + 1).join(' ');
  this.dockBorders = options.dockBorders;
  this.fullUnicode = !!options.fullUnicode;
  this.dattr = DATTR;
  this.renders = 0;
  this._ci = -1;

  this.position = {
    left: 0,
    right: 0,
    top: 0,
    bottom: 0
  };

  this.ileft = 0;
  this.itop = 0;
  this.iright = 0;
  this.ibottom = 0;
  this.iheight = 0;
  this.iwidth = 0;
  this.padding = { left: 0, top: 0, right: 0, bottom: 0 };
  this.aleft = 0;
  this.atop = 0;
  this.aright = 0;
  this.abottom = 0;
  this._handlers = {};

  this.children = [];
  this.clickable = [];
  this.keyable = [];
  this.grabKeys = false;
  this.lockKeys = false;

  this._buf = '';

  this.cursor = {
    artificial: false,
    shape: 'block',
    blink: false,
    color: null,
    _set: false,
    _state: 1,
    _hidden: true
  };
}

Screen.prototype.__proto__ = Node.prototype;
Screen.prototype.type = 'screen';
Screen.prototype.detached = false;

Screen.bind = function() {};
Screen.prototype.__defineGetter__('cols', function() {
  return this.program.cols;
});
Screen.prototype.__defineGetter__('rows', function() {
  return this.program.rows;
});
Screen.prototype.__defineGetter__('width', function() {
  return this.program.cols;
});
Screen.prototype.__defineGetter__('height', function() {
  return this.program.rows;
});

Screen.prototype.alloc = function(dirty) {
  var x, y;
  this.lines = [];
  for (y = 0; y < this.rows; y++) {
    this.lines[y] = [];
    for (x = 0; x < this.cols; x++) {
      this.lines[y][x] = [this.dattr, ' '];
    }
    this.lines[y].dirty = !!dirty;
  }
  this.olines = [];
  for (y = 0; y < this.rows; y++) {
    this.olines[y] = [];
    for (x = 0; x < this.cols; x++) {
      this.olines[y][x] = [this.dattr, ' '];
    }
  }
};

Screen.prototype.realloc = function() {
  return this.alloc();
};

Screen.prototype.render = function() {
  var self = this;

  if (this.destroyed) return;

  this._borderStops = {};

  this._ci = 0;
  this.children.forEach(function(el) {
    el.index = self._ci++;
    el.render();
  });
  this._ci = -1;

  this.draw(0, this.lines.length - 1);

  this.renders++;
};

Screen.prototype.blankLine = function(ch, dirty) {
  var out = [];
  for (var x = 0; x < this.cols; x++) {
    out[x] = [this.dattr, ch || ' '];
  }
  out.dirty = dirty;
  return out;
};

Screen.prototype.fillRegion = function(attr, ch, xi, xl, yi, yl, override) {
  var lines = this.lines
    , cell
    , xx;

  if (xi < 0) xi = 0;
  if (yi < 0) yi = 0;

  for (; yi < yl; yi++) {
    if (!lines[yi]) break;
    for (xx = xi; xx < xl; xx++) {
      cell = lines[yi][xx];
      if (!cell) break;
      if (override || attr !== cell[0] || ch !== cell[1]) {
        lines[yi][xx][0] = attr;
        lines[yi][xx][1] = ch;
        lines[yi].dirty = true;
      }
    }
  }
};

Screen.prototype.clearRegion = function(xi, xl, yi, yl, override) {
  return this.fillRegion(this.dattr, ' ', xi, xl, yi, yl, override == null ? true : override);
};

// draw — verbatim from screen.js 0.1.81 minus BCE/ACS/cuf/unicode paths
// (CONTRACTS C9; output accumulates into program buffer).
Screen.prototype.draw = function(start, end) {
  var x
    , y
    , line
    , out
    , ch
    , data
    , attr;

  var main = '';

  var lx = -1
    , ly = -1
    , o;

  if (this._buf) {
    main += this._buf;
    this._buf = '';
  }

  for (y = start; y <= end; y++) {
    line = this.lines[y];
    o = this.olines[y];

    if (!line.dirty) {
      continue;
    }
    line.dirty = false;

    out = '';
    attr = this.dattr;

    for (x = 0; x < line.length; x++) {
      data = line[x][0];
      ch = line[x][1];

      // Optimize by comparing the real output buffer to the pending output.
      if (data === o[x][0] && ch === o[x][1]) {
        if (lx === -1) {
          lx = x;
          ly = y;
        }
        continue;
      } else if (lx !== -1) {
        // parm_right_cursor (cuf) intentionally skipped — CONTRACTS C9.
        out += cup(y, x);
        lx = -1, ly = -1;
      }
      o[x][0] = data;
      o[x][1] = ch;

      if (data !== attr) {
        if (attr !== this.dattr) {
          out += '\x1b[m';
        }
        if (data !== this.dattr) {
          out += codeAttr(data);
        }
      }

      out += ch;
      attr = data;
    }

    if (attr !== this.dattr) {
      out += '\x1b[m';
    }

    if (out) {
      main += cup(y, 0) + out;
    }
  }

  if (main) {
    this.program._write(main);
  }
};


// C10 sheet emitter — per-row: cup + attr runs, from final lines, starting
// from a FRESH all-default prev buffer (everything "changed").
// Attr runs follow blessed codeAttr SGR body (bg before fg), ESC[m resets.
Screen.prototype.sheetRows = function(rows) {
  rows = rows || this.rows;
  var out = [];
  for (var y = 0; y < rows; y++) {
    var line = this.lines[y] || [];
    if (y === 23) console.error('FOOTER LINE LEN', line.length, 'cells sample', JSON.stringify(line.slice(80, 100)));
    var runs = [];
    var cur = this.dattr;
    var text = '';
    var runText = '';
    var runAttr = null;
    for (var x = 0; x < line.length; x++) {
      var a = line[x][0], ch = line[x][1];
      if (runAttr !== a) {
        if (runText !== '' ) runs.push([runAttr, runText]);
        runText = ''; runAttr = a;
      }
      runText += ch;
    }
    if (runText !== '') runs.push([runAttr, runText]);
    var s = cup(y, 0);
    var lastAttr = null;
    for (var r = 0; r < runs.length; r++) {
      if (r !== 0) s += cup(y, (function(){var p=0; for(var k=0;k<r;k++) p+=runs[k][1].length; return p;})());
      if (runs[r][0] !== this.dattr) {
        s += '\x1b[' + codeAttr(runs[r][0]) + 'm';
      }
      s += runs[r][1];
    }
    out.push(s);
  }
  return out.join('\n') + '\n';
};

// screenshot / text dump helpers (not part of the byte protocol)
Screen.prototype.screenshot = function(xi, xl, yi, yl, screen) {
  yi = yi || 0;
  if (!yl) yl = this.lines.length - yi;
  if (!xi) xi = 0;
  if (!xl) xl = this.lines[0].length - xi;
  else xl += xi;
  var res = [];
  for (var y = Math.max(yi, 0); y < yl; y++) {
    var line = '';
    if (screen) {
      if (screen === true) res.push('');
      res.push('+' + new Array(xl - xi).join('-') + '+');
    }
    for (var x = Math.max(xi, 0); x < xl; x++) {
      line += this.lines[y] && this.lines[y][x]
        ? stripTags(this.lines[y][x][1])
        : ' ';
    }
    res.push(line);
  }
  if (screen) res.push('+' + new Array(xl - xi).join('-') + '+');
  return res.join('\n');
};

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------
module.exports = {
  colors: colors,
  attrCode: attrCode,
  codeAttr: codeAttr,
  programAttr: programAttr,
  cup: cup,
  DATTR: DATTR,
  stripTags: stripTags,
  cleanTags: cleanTags,
  Node: Node,
  Element: Element,
  Box: Box,
  Text: Text,
  List: List,
  Screen: Screen
};