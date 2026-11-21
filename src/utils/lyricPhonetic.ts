import type { LyricData, LyricLine, Song } from "../types";

export type PhoneticLangType = "ja" | "ko" | "yue" | "native_phonetic" | null;

// 粤语歌词高频特征字词正则（覆盖粤语核心代词、语气助词、否定词、介词与高频口语词）
const CANTONESE_FEATURE_RE =
  /[嘅喺唔冇咁啲咗嚟睇揾搵畀俾乜嘢佢哋諗谂噃喎囉啰㗎吖咩唞嬲揀攞踎]|(?:點解|点解|邊個|边个|幾時|几时|尋日|寻日|聽日|听日|琴日|唔該|唔该|鍾意|钟意|中意|緊要|紧要|傾偈|倾偈|掛住|挂住|拍拖|邊度|边度|點樣|点样|幾多|几多|唔通|定係|定系|唔使|唔洗|唔好|唔知|唔會|唔会|係咪|系咪)/;

const CANTONESE_META_RE = /粤语|粤語|广东话|廣東話|Cantonese|粤版|粵版/i;

// 韩文谚文拆解表 (Revised Romanization of Korean)
const HANGUL_START = 0xac00;
const HANGUL_END = 0xd7a3;

const RR_CHOSEONG = [
  "g",
  "kk",
  "n",
  "d",
  "tt",
  "r",
  "m",
  "b",
  "pp",
  "s",
  "ss",
  "",
  "j",
  "jj",
  "ch",
  "k",
  "t",
  "p",
  "h",
];

const RR_JUNGSEONG = [
  "a",
  "ae",
  "ya",
  "yae",
  "eo",
  "e",
  "yeo",
  "ye",
  "o",
  "wa",
  "wae",
  "oe",
  "yo",
  "u",
  "wo",
  "we",
  "wi",
  "yu",
  "eu",
  "ui",
  "i",
];

const RR_JONGSEONG = [
  "",
  "k",
  "k",
  "k",
  "n",
  "n",
  "n",
  "t",
  "l",
  "k",
  "m",
  "p",
  "l",
  "t",
  "p",
  "l",
  "m",
  "p",
  "p",
  "t",
  "t",
  "ng",
  "t",
  "t",
  "k",
  "t",
  "p",
  "t",
];

// 日文平假名/片假名 Hepburn 罗马音映射表
const KANA_DIGRAPH_MAP: Record<string, string> = {
  きゃ: "kya",
  きゅ: "kyu",
  きょ: "kyo",
  しゃ: "sha",
  しゅ: "shu",
  しょ: "sho",
  ちゃ: "cha",
  ちゅ: "chu",
  ちょ: "cho",
  にゃ: "nya",
  にゅ: "nyu",
  にょ: "nyo",
  ひゃ: "hya",
  ひゅ: "hyu",
  ひょ: "hyo",
  みゃ: "mya",
  みゅ: "myu",
  みょ: "myo",
  りゃ: "rya",
  りゅ: "ryu",
  りょ: "ryo",
  ぎゃ: "gya",
  ぎゅ: "gyu",
  ぎょ: "gyo",
  じゃ: "ja",
  じゅ: "ju",
  じょ: "jo",
  びゃ: "bya",
  びゅ: "byu",
  びょ: "byo",
  ぴゃ: "pya",
  ぴゅ: "pyu",
  ぴょ: "pyo",
  キャ: "kya",
  キュ: "kyu",
  キョ: "kyo",
  シャ: "sha",
  シュ: "shu",
  ショ: "sho",
  チャ: "cha",
  チュ: "chu",
  チョ: "cho",
  ニャ: "nya",
  ニュ: "nyu",
  ニョ: "nyo",
  ヒャ: "hya",
  ヒュ: "hyu",
  ヒョ: "hyo",
  ミャ: "mya",
  ミュ: "myu",
  ミョ: "myo",
  リャ: "rya",
  リュ: "ryu",
  リョ: "ryo",
  ギャ: "gya",
  ギュ: "gyu",
  ギョ: "gyo",
  ジャ: "ja",
  ジュ: "ju",
  ジョ: "jo",
  ビャ: "bya",
  ビュ: "byu",
  ビョ: "byo",
  ピャ: "pya",
  ピュ: "pyu",
  ピョ: "pyo",
  ティ: "ti",
  ディ: "di",
  ファ: "fa",
  フィ: "fi",
  フェ: "fe",
  フォ: "fo",
};

const KANA_MONO_MAP: Record<string, string> = {
  あ: "a",
  い: "i",
  う: "u",
  え: "e",
  お: "o",
  か: "ka",
  き: "ki",
  く: "ku",
  け: "ke",
  こ: "ko",
  さ: "sa",
  し: "shi",
  す: "su",
  せ: "se",
  そ: "so",
  た: "ta",
  ち: "chi",
  つ: "tsu",
  て: "te",
  と: "to",
  な: "na",
  に: "ni",
  ぬ: "nu",
  ね: "ne",
  の: "no",
  は: "ha",
  ひ: "hi",
  ふ: "fu",
  へ: "he",
  ほ: "ho",
  ま: "ma",
  み: "mi",
  む: "mu",
  め: "me",
  も: "mo",
  や: "ya",
  ゆ: "yu",
  よ: "yo",
  ら: "ra",
  り: "ri",
  る: "ru",
  れ: "re",
  ろ: "ro",
  わ: "wa",
  を: "o",
  ん: "n",
  が: "ga",
  ぎ: "gi",
  ぐ: "gu",
  げ: "ge",
  ご: "go",
  ざ: "za",
  じ: "ji",
  ず: "zu",
  ぜ: "ze",
  ぞ: "zo",
  だ: "da",
  ぢ: "ji",
  づ: "zu",
  で: "de",
  ど: "do",
  ば: "ba",
  び: "bi",
  ぶ: "bu",
  べ: "be",
  ぼ: "bo",
  ぱ: "pa",
  ぴ: "pi",
  ぷ: "pu",
  ぺ: "pe",
  ぽ: "po",
  ぁ: "a",
  ぃ: "i",
  ぅ: "u",
  ぇ: "e",
  ぉ: "o",
  ゃ: "ya",
  ゅ: "yu",
  ょ: "yo",
  ア: "a",
  イ: "i",
  ウ: "u",
  エ: "e",
  オ: "o",
  カ: "ka",
  キ: "ki",
  ク: "ku",
  ケ: "ke",
  コ: "ko",
  サ: "sa",
  シ: "shi",
  ス: "su",
  セ: "se",
  ソ: "so",
  タ: "ta",
  チ: "chi",
  ツ: "tsu",
  テ: "te",
  ト: "to",
  ナ: "na",
  ニ: "ni",
  ヌ: "nu",
  ネ: "ne",
  ノ: "no",
  ハ: "ha",
  ヒ: "hi",
  フ: "fu",
  ヘ: "he",
  ホ: "ho",
  マ: "ma",
  ミ: "mi",
  ム: "mu",
  メ: "me",
  モ: "mo",
  ヤ: "ya",
  ユ: "yu",
  ヨ: "yo",
  ラ: "ra",
  リ: "ri",
  ル: "ru",
  レ: "re",
  ロ: "ro",
  ワ: "wa",
  ヲ: "o",
  ン: "n",
  ガ: "ga",
  ギ: "gi",
  グ: "gu",
  ゲ: "ge",
  ゴ: "go",
  ザ: "za",
  ジ: "ji",
  ズ: "zu",
  ゼ: "ze",
  ゾ: "zo",
  ダ: "da",
  ヂ: "ji",
  ヅ: "zu",
  デ: "de",
  ド: "do",
  バ: "ba",
  ビ: "bi",
  ブ: "bu",
  ベ: "be",
  ボ: "bo",
  パ: "pa",
  ピ: "pi",
  プ: "pu",
  ペ: "pe",
  ポ: "po",
  ァ: "a",
  ィ: "i",
  ゥ: "u",
  ェ: "e",
  ォ: "o",
  ャ: "ya",
  ュ: "yu",
  ョ: "yo",
};

// 高频粤语歌词字库 (Jyutping 粤拼无声调优雅展示版)
const CANTONESE_JYUTPING_MAP: Record<string, string> = {
  我: "ngo",
  你: "nei",
  佢: "keoi",
  哋: "dei",
  嘅: "ge",
  喺: "hai",
  係: "hai",
  系: "hai",
  唔: "m",
  冇: "mou",
  咁: "gam",
  啲: "di",
  咗: "zo",
  嚟: "lai",
  來: "loi",
  来: "loi",
  睇: "tai",
  揾: "wan",
  搵: "wan",
  畀: "bei",
  俾: "bei",
  乜: "mat",
  嘢: "je",
  仲: "zung",
  點: "dim",
  点: "dim",
  解: "gaai",
  邊: "bin",
  边: "bin",
  個: "go",
  个: "go",
  幾: "gei",
  几: "gei",
  時: "si",
  时: "si",
  尋: "cam",
  寻: "cam",
  日: "jat",
  聽: "ting",
  听: "ting",
  琴: "kam",
  該: "goi",
  该: "goi",
  鍾: "zung",
  钟: "zung",
  中: "zung",
  意: "ji",
  緊: "gan",
  紧: "gan",
  要: "jiu",
  傾: "king",
  倾: "king",
  偈: "gai",
  掛: "gwaa",
  挂: "gwaa",
  住: "zyu",
  拍: "paak",
  拖: "to",
  愛: "oi",
  爱: "oi",
  情: "cing",
  心: "sam",
  人: "jan",
  生: "saang",
  風: "fung",
  风: "fung",
  雨: "jyu",
  夜: "je",
  天: "tin",
  空: "hung",
  海: "hoi",
  闊: "fut",
  阔: "fut",
  山: "saan",
  水: "seoi",
  月: "jyut",
  光: "gwong",
  星: "sing",
  雲: "wan",
  云: "wan",
  花: "faa",
  落: "lok",
  開: "hoi",
  开: "hoi",
  笑: "siu",
  哭: "huk",
  淚: "leoi",
  泪: "leoi",
  眼: "ngaan",
  神: "san",
  手: "sau",
  放: "fong",
  走: "zau",
  行: "haang",
  過: "gwo",
  过: "gwo",
  去: "heoi",
  回: "wui",
  頭: "tau",
  头: "tau",
  望: "mong",
  見: "gin",
  见: "gin",
  想: "soeng",
  念: "nim",
  記: "gei",
  记: "gei",
  億: "jik",
  忆: "jik",
  憶: "jik",
  忘: "mong",
  痛: "tung",
  傷: "soeng",
  伤: "soeng",
  悲: "bei",
  歡: "fun",
  欢: "fun",
  喜: "hei",
  樂: "lok",
  乐: "lok",
  歌: "go",
  聲: "sing",
  声: "sing",
  音: "jam",
  唱: "coeng",
  夢: "mung",
  梦: "mung",
  醒: "sing",
  醉: "zeoi",
  酒: "zau",
  杯: "bui",
  茶: "caa",
  街: "gaai",
  路: "lou",
  遠: "jyun",
  远: "jyun",
  近: "gan",
  長: "coeng",
  长: "coeng",
  短: "dyun",
  高: "gou",
  低: "dai",
  深: "sam",
  淺: "cin",
  浅: "cin",
  冷: "laang",
  暖: "nyun",
  熱: "jit",
  热: "jit",
  凍: "dung",
  冻: "dung",
  雪: "syut",
  霜: "soeng",
  春: "ceon",
  夏: "haa",
  秋: "cau",
  冬: "dung",
  年: "nin",
  歲: "seoi",
  岁: "seoi",
  今: "gam",
  明: "ming",
  昨: "zok",
  早: "zou",
  晚: "maan",
  朝: "ziu",
  夕: "zik",
  世: "sai",
  界: "gaai",
  間: "gaan",
  间: "gaan",
  紅: "hung",
  红: "hung",
  塵: "can",
  尘: "can",
  緣: "jyun",
  缘: "jyun",
  份: "fan",
  分: "fan",
  合: "hap",
  聚: "zeoi",
  散: "saan",
  離: "lei",
  离: "lei",
  別: "bit",
  别: "bit",
  難: "naan",
  难: "naan",
  易: "ji",
  真: "zan",
  假: "gaa",
  對: "deoi",
  对: "deoi",
  錯: "co",
  错: "co",
  好: "hou",
  壞: "waai",
  坏: "waai",
  美: "mei",
  麗: "lai",
  丽: "lai",
  靚: "leng",
  靓: "leng",
  誰: "seoi",
  谁: "seoi",
  何: "ho",
  處: "cyu",
  处: "cyu",
  怎: "zam",
  麼: "mo",
  么: "mo",
  樣: "joeng",
  样: "joeng",
  若: "joek",
  如: "jyu",
  果: "gwo",
  但: "daan",
  只: "zi",
  是: "si",
  非: "fei",
  無: "mou",
  无: "mou",
  有: "jau",
  在: "zoi",
  再: "zoi",
  又: "jau",
  也: "jaa",
  都: "dou",
  更: "gang",
  最: "zeoi",
  很: "han",
  太: "taai",
  全: "cyun",
  部: "bou",
  一: "jat",
  二: "ji",
  三: "saam",
  四: "sei",
  五: "ng",
  六: "luk",
  七: "cat",
  八: "baat",
  九: "gau",
  十: "sap",
  百: "baak",
  千: "cin",
  萬: "maan",
  万: "maan",
  零: "ling",
  半: "bun",
  雙: "soeng",
  双: "soeng",
  單: "daan",
  单: "daan",
  孤: "gu",
  獨: "duk",
  独: "duk",
  寂: "zik",
  寞: "mok",
  陪: "pui",
  伴: "bun",
  隨: "ceoi",
  随: "ceoi",
  追: "zeoi",
  等: "dang",
  待: "doi",
  期: "kei",
  盼: "paan",
  希: "hei",
  願: "jyun",
  愿: "jyun",
  留: "lau",
  戀: "lyun",
  恋: "lyun",
  捨: "se",
  舍: "se",
  得: "dak",
  失: "sat",
  知: "zi",
  道: "dou",
  白: "baak",
  懂: "dung",
  信: "seon",
  任: "jam",
  疑: "ji",
  問: "man",
  问: "man",
  答: "daap",
  話: "waa",
  话: "waa",
  講: "gong",
  讲: "gong",
  說: "syut",
  说: "syut",
  語: "jyu",
  语: "jyu",
  言: "jin",
  字: "zi",
  句: "geoi",
  書: "syu",
  书: "syu",
  相: "soeng",
  思: "si",
  牽: "hin",
  牵: "hin",
  擁: "jung",
  拥: "jung",
  抱: "pou",
  吻: "man",
  唇: "seon",
  臉: "lim",
  脸: "lim",
  眉: "mei",
  髮: "faat",
  发: "faat",
  身: "san",
  影: "jing",
  步: "bou",
  腳: "goek",
  脚: "goek",
  飛: "fei",
  飞: "fei",
  翔: "coeng",
  跳: "tiu",
  舞: "mou",
  轉: "zyun",
  转: "zyun",
  變: "bin",
  变: "bin",
  化: "faa",
  成: "sing",
  敗: "baai",
  败: "baai",
  勝: "sing",
  胜: "sing",
  負: "fu",
  负: "fu",
  強: "koeng",
  强: "koeng",
  弱: "joek",
  勇: "jung",
  敢: "gam",
  怕: "paa",
  驚: "ging",
  惊: "ging",
  慌: "fong",
  亂: "lyun",
  乱: "lyun",
  靜: "zing",
  静: "zing",
  安: "on",
  寧: "ning",
  宁: "ning",
  平: "ping",
  凡: "faan",
  常: "soeng",
  特: "dak",
};

/**
 * 智能检测当前歌曲是否属于粤语、日语、韩语等需要音译的语种
 */
export function detectPhoneticLanguage(
  lyric: LyricData | null | undefined,
  song: Song | null | undefined
): PhoneticLangType {
  if (!lyric || !lyric.lines || lyric.lines.length === 0) return null;

  let kanaCount = 0;
  let hangulCount = 0;
  let cantoneseFeatureHits = 0;
  let nativePhoneticLines = 0;

  for (const line of lyric.lines) {
    const txt = line.text || "";
    if (line.phonetic && line.phonetic.trim().length > 0) {
      nativePhoneticLines++;
    }
    if (CANTONESE_FEATURE_RE.test(txt)) {
      cantoneseFeatureHits++;
    }
    for (let i = 0; i < txt.length; i++) {
      const code = txt.charCodeAt(i);
      if (
        (code >= 0x3040 && code <= 0x309f) ||
        (code >= 0x30a0 && code <= 0x30ff) ||
        (code >= 0x31f0 && code <= 0x31ff)
      ) {
        kanaCount++;
      } else if (
        (code >= 0xac00 && code <= 0xd7af) ||
        (code >= 0x1100 && code <= 0x11ff) ||
        (code >= 0x3130 && code <= 0x318f)
      ) {
        hangulCount++;
      }
    }
  }

  // 1. 韩语判定：存在谚文字符
  if (hangulCount >= 2) return "ko";

  // 2. 日语判定：存在平假名或片假名
  if (kanaCount >= 2) return "ja";

  // 3. 粤语判定：歌词命中粤语特征词，或歌名/专辑含粤语标识
  const metaStr = `${song?.title || ""} ${song?.album || ""}`;
  if (cantoneseFeatureHits >= 1 || CANTONESE_META_RE.test(metaStr)) {
    return "yue";
  }

  // 4. KRC 原生内嵌音译层（如泰语、俄语或全书面语粤语歌自带 KRC 音译层）
  if (lyric.hasPhonetic || nativePhoneticLines >= 2) {
    return "native_phonetic";
  }

  return null;
}

function romanizeKoreanText(text: string): string {
  const words: string[] = [];
  let currentSyllables: string[] = [];

  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    if (code >= HANGUL_START && code <= HANGUL_END) {
      const sIdx = code - HANGUL_START;
      const cho = Math.floor(sIdx / 588);
      const jung = Math.floor((sIdx % 588) / 28);
      const jong = sIdx % 28;
      const rom =
        (RR_CHOSEONG[cho] || "") +
        (RR_JUNGSEONG[jung] || "") +
        (RR_JONGSEONG[jong] || "");
      currentSyllables.push(rom);
    } else if (/\s/.test(text[i])) {
      if (currentSyllables.length > 0) {
        words.push(currentSyllables.join(" "));
        currentSyllables = [];
      }
    } else if (/[a-zA-Z0-9]/.test(text[i])) {
      if (currentSyllables.length > 0) {
        words.push(currentSyllables.join(" "));
        currentSyllables = [];
      }
      let latin = text[i];
      while (i + 1 < text.length && /[a-zA-Z0-9']/.test(text[i + 1])) {
        i++;
        latin += text[i];
      }
      words.push(latin.toLowerCase());
    }
  }
  if (currentSyllables.length > 0) {
    words.push(currentSyllables.join(" "));
  }
  return words.join("  ").trim();
}

function romanizeJapaneseText(text: string): string {
  const tokens: string[] = [];
  let i = 0;
  while (i < text.length) {
    const ch = text[i];
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    // 促音 っ / ッ
    if (ch === "っ" || ch === "ッ") {
      const next2 = text.slice(i + 1, i + 3);
      const next1 = text[i + 1] || "";
      const nextRom = KANA_DIGRAPH_MAP[next2] || KANA_MONO_MAP[next1] || "";
      if (nextRom) {
        tokens.push(nextRom[0] + nextRom);
        i += KANA_DIGRAPH_MAP[next2] ? 3 : 2;
        continue;
      }
      i++;
      continue;
    }
    // 双字符拗音
    const sub2 = text.slice(i, i + 2);
    if (KANA_DIGRAPH_MAP[sub2]) {
      tokens.push(KANA_DIGRAPH_MAP[sub2]);
      i += 2;
      continue;
    }
    // 单假名
    if (KANA_MONO_MAP[ch]) {
      tokens.push(KANA_MONO_MAP[ch]);
      i++;
      continue;
    }
    // 汉字若在常用粤拼/汉音表内则回退音读，否则跳过标点
    if (CANTONESE_JYUTPING_MAP[ch]) {
      tokens.push(CANTONESE_JYUTPING_MAP[ch]);
    }
    i++;
  }
  return tokens.join(" ").trim();
}

function romanizeCantoneseText(text: string): string {
  const tokens: string[] = [];
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (CANTONESE_JYUTPING_MAP[ch]) {
      tokens.push(CANTONESE_JYUTPING_MAP[ch]);
    } else if (/[a-zA-Z]/.test(ch)) {
      let word = ch;
      while (i + 1 < text.length && /[a-zA-Z']/.test(text[i + 1])) {
        i++;
        word += text[i];
      }
      tokens.push(word.toLowerCase());
    }
  }
  return tokens.join(" ").trim();
}

/**
 * 获取单行歌词的音译文本（优先使用 KRC 原生音译，缺失时根据粤/日/韩语种智能生成）
 */
export function resolveLinePhoneticText(
  line: LyricLine,
  lang: PhoneticLangType
): string {
  if (!lang) return "";
  if (line.phonetic && line.phonetic.trim().length > 0) {
    return line.phonetic.trim();
  }
  const raw = (line.text || "").trim();
  if (!raw) return "";
  // 过滤作词/作曲/编曲等元信息行
  if (/^(作词|作曲|编曲|制作|监制|混音|吉他|和声|词|曲|Arranger|Composer|Lyricist)\s*[:：]/i.test(raw)) {
    return "";
  }
  if (lang === "ko") {
    return romanizeKoreanText(raw);
  }
  if (lang === "ja") {
    return romanizeJapaneseText(raw);
  }
  if (lang === "yue") {
    return romanizeCantoneseText(raw);
  }
  return "";
}
