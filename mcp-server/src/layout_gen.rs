pub struct FieldInfo {
    pub name: String,
    pub id: u64,
    pub repetitions: u32,
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

pub fn generate(table: &str, fields: &[FieldInfo]) -> String {
    const ROW_H: f64 = 32.0;
    const Y0: f64 = 20.0;

    let bottom = Y0 + fields.len() as f64 * ROW_H;
    let mut out = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <fmxmlsnippet type=\"LayoutObjectList\">\n  \
         <Layout enclosingRectTop=\"{Y0:.7}\" enclosingRectLeft=\"20.0000000\" \
         enclosingRectBottom=\"{bottom:.7}\" enclosingRectRight=\"420.0000000\">\n"
    );

    let mut key = 100u32;
    for (i, f) in fields.iter().enumerate() {
        let top = Y0 + i as f64 * ROW_H;
        let bot = top + ROW_H;
        let fname = xml_escape(&f.name);
        let tname = xml_escape(table);
        out.push_str(&label_object(key, top, bot, &fname));
        out.push_str(&field_object(key + 1, top, bot, &fname, &tname, f.id, f.repetitions));
        key += 2;
    }

    out.push_str("  </Layout>\n</fmxmlsnippet>\n");
    out
}

const LABEL_TEMPLATE: &str = r##"    <Object type="Text" key="{{KEY}}" LabelKey="0" flags="0" rotation="0">
      <Bounds top="{{TOP}}" left="20.0000000" bottom="{{BOTTOM}}" right="220.0000000"/>
      <TextObj flags="2">
        <ExtendedAttributes fontHeight="0" graphicFormat="0">
          <NumFormat flags="0" charStyle="0" negativeStyle="0" currencySymbol="" thousandsSep="0" decimalPoint="0" negativeColor="#0" decimalDigits="0" trueString="" falseString=""/>
          <DateFormat format="0" charStyle="0" monthStyle="0" dayStyle="0" separator="0">
            <DateElement>0</DateElement>
            <DateElement>0</DateElement>
            <DateElement>0</DateElement>
            <DateElement>0</DateElement>
            <DateElementSep index="0"/>
            <DateElementSep index="1"/>
            <DateElementSep index="2"/>
            <DateElementSep index="3"/>
            <DateElementSep index="4"/>
          </DateFormat>
          <TimeFormat flags="0" charStyle="0" hourStyle="0" minsecStyle="0" separator="0" amString="" pmString="" ampmString=""/>
          <CharacterStyle mask="32695">
            <Font-family codeSet="Other" fontId="16" postScript="MeiryoUI">Meiryo UI</Font-family>
            <Font-size>14</Font-size>
            <Face>0</Face>
            <Color>#333333</Color>
          </CharacterStyle>
        </ExtendedAttributes>
        <Styles>
        <LocalCSS>
			self:normal .self
			{
				font-family: -fm-font-family(Meiryo UI,MeiryoUI);
				font-size: 14pt;
			}
		</LocalCSS>
		<FullCSS>
			self:normal .self
			{
				background-image: none;
				background-position: 0% 0%;
				background-size: auto;
				background-repeat: repeat repeat;
				background-origin: padding-box;
				background-clip: border-box;
				background-color: rgba(94.902%,94.902%,94.902%,0.6);
				border-top-color: rgba(80%,80%,80%,1);
				border-right-color: rgba(80%,80%,80%,1);
				border-bottom-color: rgba(80%,80%,80%,1);
				border-left-color: rgba(80%,80%,80%,1);
				border-top-style: solid;
				border-right-style: solid;
				border-bottom-style: solid;
				border-left-style: solid;
				border-top-width: 1pt;
				border-right-width: 1pt;
				border-bottom-width: 1pt;
				border-left-width: 1pt;
				border-top-right-radius: 0pt 0pt;
				border-bottom-right-radius: 0pt 0pt;
				border-bottom-left-radius: 0pt 0pt;
				border-top-left-radius: 0pt 0pt;
				border-image-source: none;
				border-image-slice: 100% 100% 100% 100% fill;
				border-image-width: 1 1 1 1;
				border-image-outset: 0 0 0 0;
				border-image-repeat: stretch stretch;
				outline-width: 0pt;
				outline-style: none;
				outline-color: invert;
				outline-offset: 0pt;
				font-family: -fm-font-family(Meiryo UI,MeiryoUI);
				font-weight: normal;
				font-stretch: normal;
				font-style: normal;
				font-variant: normal;
				font-size: 14pt;
				color: rgba(20%,20%,20%,1);
				direction: ltr;
				line-height: 1line;
				block-progression: tb;
				text-align: right;
				text-transform: none;
				text-indent: 0pt;
				box-shadow: none;
				box-sizing: content-box;
				vertical-align: baseline;
				-fm-digit-set: roman;
				-fm-space-before: 0line;
				-fm-space-after: 0line;
				-fm-tab-stops: ;
				-fm-strikethrough: false;
				-fm-underline: none;
				-fm-glyph-variant: ;
				-fm-paragraph-margin-left: 0pt;
				-fm-paragraph-margin-right: 0pt;
				-fm-character-direction: ;
				-fm-use-default-appearance: false;
				-fm-override-with-classic: false;
				-fm-baseline-shift: 0pt;
				-fm-fill-effect: 0;
				-fm-highlight-color: rgba(0%,0%,0%,0);
				-fm-text-vertical-align: center;
				-fm-tategaki: false;
				-fm-rotation: 0;
				-fm-borders-between-reps: false;
				-fm-borders-baseline: false;
				-fm-texty-field: false;
				-fm-box-shadow-persist: none;
			}
			self:normal .inner_border
			{
				border-top-color: rgba(0%,0%,0%,0);
				border-right-color: rgba(0%,0%,0%,0);
				border-bottom-color: rgba(0%,0%,0%,0);
				border-left-color: rgba(0%,0%,0%,0);
				border-top-style: none;
				border-right-style: none;
				border-bottom-style: none;
				border-left-style: none;
				border-top-width: 0pt;
				border-right-width: 0pt;
				border-bottom-width: 0pt;
				border-left-width: 0pt;
				border-top-right-radius: 0pt 0pt;
				border-bottom-right-radius: 0pt 0pt;
				border-bottom-left-radius: 0pt 0pt;
				border-top-left-radius: 0pt 0pt;
				padding-top: 0.25em;
				padding-right: 0.5em;
				padding-bottom: 0.17em;
				padding-left: 0.5em;
				margin-top: 0pt;
				margin-right: 0pt;
				margin-bottom: 0pt;
				margin-left: 0pt;
				width: auto;
				height: auto;
				top: auto;
				right: auto;
				bottom: auto;
				left: auto;
				position: static;
				box-shadow: none;
				box-sizing: content-box;
			}
			self:normal .text
			{
				padding-top: 0pt;
				padding-right: 0pt;
				padding-bottom: 0pt;
				padding-left: 0pt;
				margin-top: 0pt;
				margin-right: 0pt;
				margin-bottom: 0pt;
				margin-left: 0pt;
				height: 100%;
				box-sizing: border-box;
			}
			self:normal .baseline
			{
				border-top-color: rgba(0%,0%,0%,0);
				border-right-color: rgba(0%,0%,0%,0);
				border-bottom-color: rgba(0%,0%,0%,0);
				border-left-color: rgba(0%,0%,0%,0);
				border-top-style: none;
				border-right-style: none;
				border-bottom-style: none;
				border-left-style: none;
				border-top-width: 0pt;
				border-right-width: 0pt;
				border-bottom-width: 0pt;
				border-left-width: 0pt;
			}
		</FullCSS>
          <ThemeName>jp.co.genecom.theme.SourceTableStyle</ThemeName>
        </Styles>
        <CharacterStyleVector>
          <Style>
            <Data>{{FIELD_NAME}}</Data>
            <CharacterStyle mask="32695">
              <Font-family codeSet="Other" fontId="16" postScript="MeiryoUI">Meiryo UI</Font-family>
              <Font-size>14</Font-size>
              <Face>0</Face>
              <Color>#333333</Color>
            </CharacterStyle>
          </Style>
        </CharacterStyleVector>
        <ParagraphStyleVector>
          <Style>
            <Data>{{FIELD_NAME}}</Data>
            <ParagraphStyle mask="0">
			</ParagraphStyle>
          </Style>
        </ParagraphStyleVector>
      </TextObj>
    </Object>
"##;

const FIELD_TEMPLATE: &str = r##"    <Object type="Field" key="{{KEY}}" LabelKey="0" name="{{FIELD_NAME}}" flags="0" rotation="0">
      <Bounds top="{{TOP}}" left="220.0000000" bottom="{{BOTTOM}}" right="420.0000000"/>
      <FieldObj numOfReps="1" flags="1048608" inputMode="0" keyboardType="1" displayType="0" quickFind="1" pictFormat="5">
        <Name>{{TABLE_NAME}}::{{FIELD_NAME}}</Name>
        <ExtendedAttributes fontHeight="14" graphicFormat="5">
          <NumFormat flags="2304" charStyle="0" negativeStyle="0" currencySymbol="¥" thousandsSep="44" decimalPoint="46" negativeColor="#DD000000" decimalDigits="2" trueString="はい" falseString="いいえ"/>
          <DateFormat format="0" charStyle="0" monthStyle="1" dayStyle="1" separator="47">
            <DateElement>8</DateElement>
            <DateElement>4</DateElement>
            <DateElement>1</DateElement>
            <DateElement>3</DateElement>
            <DateElementSep index="0"/>
            <DateElementSep index="1">年</DateElementSep>
            <DateElementSep index="2">月</DateElementSep>
            <DateElementSep index="3">日 </DateElementSep>
            <DateElementSep index="4"/>
          </DateFormat>
          <TimeFormat flags="159" charStyle="0" hourStyle="0" minsecStyle="1" separator="58" amString=" 午前" pmString=" 午後" ampmString=""/>
          <CharacterStyle mask="32695">
            <Font-family codeSet="Other" fontId="16" postScript="MeiryoUI">Meiryo UI</Font-family>
            <Font-size>14</Font-size>
            <Face>0</Face>
            <Color>#333333</Color>
          </CharacterStyle>
        </ExtendedAttributes>
        <Styles>
          <LocalCSS>
			self:normal .self
			{
				font-family: -fm-font-family(Meiryo UI,MeiryoUI);
				font-size: 14pt;
			}
			</LocalCSS>
			<FullCSS>
			self:normal .self
			{
				background-image: none;
				background-position: 0% 0%;
				background-size: auto;
				background-repeat: repeat repeat;
				background-origin: padding-box;
				background-clip: border-box;
				background-color: rgba(100%,100%,100%,1);
				border-top-color: rgba(80%,80%,80%,1);
				border-right-color: rgba(80%,80%,80%,1);
				border-bottom-color: rgba(80%,80%,80%,1);
				border-left-color: rgba(80%,80%,80%,1);
				border-top-style: solid;
				border-right-style: solid;
				border-bottom-style: solid;
				border-left-style: solid;
				border-top-width: 1pt;
				border-right-width: 1pt;
				border-bottom-width: 1pt;
				border-left-width: 1pt;
				border-top-right-radius: 0pt 0pt;
				border-bottom-right-radius: 0pt 0pt;
				border-bottom-left-radius: 0pt 0pt;
				border-top-left-radius: 0pt 0pt;
				border-image-source: none;
				border-image-slice: 100% 100% 100% 100% fill;
				border-image-width: 1 1 1 1;
				border-image-outset: 0 0 0 0;
				border-image-repeat: stretch stretch;
				outline-width: 0pt;
				outline-style: none;
				outline-color: invert;
				outline-offset: 0pt;
				font-family: -fm-font-family(Meiryo UI,MeiryoUI);
				font-weight: normal;
				font-stretch: normal;
				font-style: normal;
				font-variant: normal;
				font-size: 14pt;
				color: rgba(20%,20%,20%,1);
				direction: ltr;
				line-height: 1line;
				block-progression: tb;
				text-align: left;
				text-transform: none;
				text-indent: 0pt;
				box-shadow: none;
				box-sizing: content-box;
				vertical-align: baseline;
				-fm-digit-set: roman;
				-fm-space-before: 0line;
				-fm-space-after: 0line;
				-fm-tab-stops: ;
				-fm-strikethrough: false;
				-fm-underline: none;
				-fm-glyph-variant: ;
				-fm-paragraph-margin-left: 0pt;
				-fm-paragraph-margin-right: 0pt;
				-fm-character-direction: ;
				-fm-use-default-appearance: false;
				-fm-override-with-classic: false;
				-fm-baseline-shift: 0pt;
				-fm-fill-effect: 0;
				-fm-highlight-color: rgba(0%,0%,0%,0);
				-fm-text-vertical-align: center;
				-fm-tategaki: false;
				-fm-rotation: 0;
				-fm-borders-between-reps: false;
				-fm-borders-baseline: false;
				-fm-texty-field: false;
				-fm-box-shadow-persist: none;
			}
			self:focus .self
			{
				border-top-color: rgba(18.8235%,51.3726%,98.4314%,1);
				border-right-color: rgba(18.8235%,51.3726%,98.4314%,1);
				border-bottom-color: rgba(18.8235%,51.3726%,98.4314%,1);
				border-left-color: rgba(18.8235%,51.3726%,98.4314%,1);
				box-shadow: 0pt 0pt 2pt 1pt rgba(18.8235%,51.3726%,98.4314%,0.3);
			}
			self:placeholder .self
			{
				color: rgba(80%,80%,80%,1);
			}
			self:normal .inner_border
			{
				border-top-color: rgba(0%,0%,0%,0);
				border-right-color: rgba(0%,0%,0%,0);
				border-bottom-color: rgba(0%,0%,0%,0);
				border-left-color: rgba(0%,0%,0%,0);
				border-top-style: none;
				border-right-style: none;
				border-bottom-style: none;
				border-left-style: none;
				border-top-width: 0pt;
				border-right-width: 0pt;
				border-bottom-width: 0pt;
				border-left-width: 0pt;
				border-top-right-radius: 0pt 0pt;
				border-bottom-right-radius: 0pt 0pt;
				border-bottom-left-radius: 0pt 0pt;
				border-top-left-radius: 0pt 0pt;
				padding-top: 0pt;
				padding-right: 0pt;
				padding-bottom: 0pt;
				padding-left: 0pt;
				margin-top: 0pt;
				margin-right: 0pt;
				margin-bottom: 0pt;
				margin-left: 0pt;
				width: auto;
				height: auto;
				top: auto;
				right: auto;
				bottom: auto;
				left: auto;
				position: static;
				box-shadow: none;
				box-sizing: content-box;
			}
			self:normal .text
			{
				margin-top: 0.37em;
				margin-right: 0.37em;
				margin-bottom: 0.05em;
				margin-left: 0.37em;
				height: auto;
				top: 0pt;
				right: 0pt;
				bottom: 0pt;
				left: 0pt;
				position: absolute;
				box-sizing: border-box;
			}
			self:normal .baseline
			{
				border-top-color: rgba(0%,0%,0%,0);
				border-right-color: rgba(0%,0%,0%,0);
				border-bottom-color: rgba(0%,0%,0%,0);
				border-left-color: rgba(0%,0%,0%,0);
				border-top-style: none;
				border-right-style: none;
				border-bottom-style: none;
				border-left-style: none;
				border-top-width: 0pt;
				border-right-width: 0pt;
				border-bottom-width: 1pt;
				border-left-width: 0pt;
			}
			self:normal .repeat_border
			{
				border-top-color: rgba(80%,80%,80%,1);
				border-right-color: rgba(80%,80%,80%,1);
				border-bottom-color: rgba(80%,80%,80%,1);
				border-left-color: rgba(80%,80%,80%,1);
				border-top-style: solid;
				border-right-style: solid;
				border-bottom-style: solid;
				border-left-style: solid;
				border-top-width: 1pt;
				border-right-width: 1pt;
				border-bottom-width: 1pt;
				border-left-width: 1pt;
			}
			</FullCSS>
          <ThemeName>jp.co.genecom.theme.SourceTableStyle</ThemeName>
        </Styles>
        <DDRInfo>
          <Field name="{{FIELD_NAME}}" id="{{FIELD_ID}}" repetition="1" maxRepetition="{{MAX_REPS}}" table="{{TABLE_NAME}}"/>
        </DDRInfo>
      </FieldObj>
    </Object>
"##;

fn label_object(key: u32, top: f64, bottom: f64, field_name: &str) -> String {
    LABEL_TEMPLATE
        .replace("{{KEY}}", &key.to_string())
        .replace("{{TOP}}", &format!("{top:.7}"))
        .replace("{{BOTTOM}}", &format!("{bottom:.7}"))
        .replace("{{FIELD_NAME}}", field_name)
}

fn field_object(key: u32, top: f64, bottom: f64, field_name: &str, table_name: &str, id: u64, repetitions: u32) -> String {
    FIELD_TEMPLATE
        .replace("{{KEY}}", &key.to_string())
        .replace("{{TOP}}", &format!("{top:.7}"))
        .replace("{{BOTTOM}}", &format!("{bottom:.7}"))
        .replace("{{FIELD_NAME}}", field_name)
        .replace("{{TABLE_NAME}}", table_name)
        .replace("{{FIELD_ID}}", &id.to_string())
        .replace("{{MAX_REPS}}", &repetitions.to_string())
}
