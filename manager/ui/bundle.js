import { append, bindProp, component, element, insert, merge, prop, setup, spread, text } from "nsh:rt";
import { effect, mount, nativeComponent, signal } from "nsh";
import { Window, titleBar } from "nsh/desktop";
import { platform } from "os";

//#region node_modules/@makekosmos/imago-nsh/crates/imago-nsh/ui/types.ts
/** Metrics of the settings shell the Manager and NSH preference panes wear. */
const settingsMetrics = {
	sidebarWidth: 282,
	sidebarPadding: 8,
	sidebarTop: 16,
	searchHeight: 32,
	searchGap: 8,
	navRowHeight: 40,
	navGroupGap: 14,
	navEntryGap: 2,
	tileSize: 24,
	tileRadius: 6,
	logoRowHeight: 50,
	logoRowPadding: 7,
	logoTileSize: 36,
	cardRadius: 10,
	cardRowHeight: 51,
	cardDenseRowHeight: 50,
	cardPaddingX: 10,
	cardPaddingY: 8,
	cardDensePaddingY: 6,
	fieldRowHeight: 48,
	fieldLabelWidth: 62,
	pageGutter: 16,
	pageMaxWidth: 1180,
	pageTop: 12,
	pageBottom: 56,
	headerHeight: 26,
	headerGap: 18,
	sectionGap: 40,
	labelGap: 8
};

//#endregion
//#region node_modules/@makekosmos/imago-nsh/crates/imago-nsh/ui/icons.ts
const icons = {
	mundus: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%20854%20841%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M848.85895%20312.15771C833.823%20246.3922%20789.67505%20200.34573%20750.2724%20209.29922C719.13416%20216.38741%20414.19922%20453.55243%20336.24701%20504.18228C403.00226%20439.42935%20679.25177%20166.39703%20693.11469%20137.61795C710.65662%20101.21774%20675.57306%2047.923195%20614.73621%2018.664467C553.89935%20-10.594261%20490.39661%20-4.8384075%20472.80139%2031.615091C458.93848%2060.394169%20320.19763%20399.6716%20311.18674%20492.19101C302.17584%20399.6716%20394.95541%2060.447464%20381.0925%2031.615091C363.5506%20-4.8384075%20299.99451%20-10.594261%20239.15767%2018.664467C178.32083%2047.923195%20143.23724%20101.21774%20160.77916%20137.61795C174.64207%20166.39703%20219.37186%20439.48267%20286.12711%20504.18228C208.12158%20453.55243%20134.75906%20216.38741%20103.62083%20209.29922C64.164856%20200.29243%2020.016878%20246.3389%205.0342722%20312.15771C-10.001652%20377.92322%209.7798576%20438.57248%2049.235832%20447.57925C80.374062%20454.66742%20187.75366%20543.08728%20279.99533%20531.3092C191.75259%20560.6745%2094.663284%20517.02222%2069.656731%20536.95441C38.038635%20562.16272%2046.516647%20625.37006%2088.638565%20678.13171C130.70715%20730.89337%20190.47778%20753.22388%20222.09589%20728.01556C247.10245%20708.08337%20249.01703%20632.46222%20297.32394%20553C265.2793%20640.29651%20304.74017%20735.79651%20304.74017%20767.77325C304.74017%20808.22388%20359.44498%20841%20427%20841C494.55502%20841%20549.2605%20808.22388%20549.2605%20767.77325C549.2605%20735.84979%20357.2009%20640.29651%20325.15625%20553C373.46317%20632.46222%20606.89819%20708.08337%20631.90472%20728.01556C663.52283%20753.22388%20723.29346%20730.89337%20765.36206%20678.13171C807.43066%20625.37006%20815.96136%20562.16272%20784.34326%20536.95441C759.33673%20517.02222%20430.72763%20560.6745%20342.48489%20531.3092C434.72656%20543.08728%20773.57263%20454.66742%20804.76416%20447.57925C844.22015%20438.57248%20864.00165%20377.97653%20848.9657%20312.15771L848.85895%20312.15771Z%22%20fill%3D%22currentColor%22%20fill-rule%3D%22nonzero%22%2F%3E%3C%2Fsvg%3E",
	about: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M16%202V6M8%202V6%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M13%204H11C7.22876%204%205.34315%204%204.17157%205.17157C3%206.34315%203%208.22876%203%2012V14C3%2017.7712%203%2019.6569%204.17157%2020.8284C5.34315%2022%207.22876%2022%2011%2022H13C16.7712%2022%2018.6569%2022%2019.8284%2020.8284C21%2019.6569%2021%2017.7712%2021%2014V12C21%208.22876%2021%206.34315%2019.8284%205.17157C18.6569%204%2016.7712%204%2013%204Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3%2010H21%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M10%2018.5002L9.99999%2013.8474C9.99999%2013.6557%209.86325%2013.5002%209.69458%2013.5002H9M14%2018.4983L15.4855%2013.8923C15.4951%2013.8626%2015.5%2013.8315%2015.5%2013.8002C15.5%2013.6346%2015.3657%2013.5002%2015.2%2013.5002L13%2013.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	check: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M5%2013.2592L7.58583%2015.9568C8.2525%2016.6523%208.58583%2017%209.00004%2017C9.41425%2017%209.74759%2016.6523%2010.4143%2015.9568L19%207%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	inbox: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M2.5%2012C2.5%207.52166%202.5%205.28249%203.89124%203.89124C5.28249%202.5%207.52166%202.5%2012%202.5C16.4783%202.5%2018.7175%202.5%2020.1088%203.89124C21.5%205.28249%2021.5%207.52166%2021.5%2012C21.5%2016.4783%2021.5%2018.7175%2020.1088%2020.1088C18.7175%2021.5%2016.4783%2021.5%2012%2021.5C7.52166%2021.5%205.28249%2021.5%203.89124%2020.1088C2.5%2018.7175%202.5%2016.4783%202.5%2012Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M21.5%2013.5H16.5743C15.7322%2013.5%2015.0706%2014.2036%2014.6995%2014.9472C14.2963%2015.7551%2013.4889%2016.5%2012%2016.5C10.5111%2016.5%209.70373%2015.7551%209.30054%2014.9472C8.92942%2014.2036%208.26777%2013.5%207.42566%2013.5H2.5%22%20stroke%3D%22currentColor%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	logbook: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M7.99978%203.5H6.60021C4.43183%203.5%203.34764%203.5%202.67399%204.17362C2.00034%204.84724%202.00029%205.93144%202.00021%208.09982L2%2013.3998C1.99992%2015.5684%201.99987%2016.6526%202.67353%2017.3263C3.34719%2018%204.43146%2018%206.6%2018H8.95042C10.4329%2018%2011.7092%2019.0464%2011.9999%2020.5V5.5C11.0556%204.24097%209.99989%203.5%207.99978%203.5Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M16.0001%203.5H17.3997C19.5681%203.5%2020.6523%203.5%2021.3259%204.17362C21.9996%204.84724%2021.9996%205.93144%2021.9997%208.09982L21.9999%2013.3998C22%2015.5684%2022%2016.6526%2021.3264%2017.3263C20.6527%2018%2019.5684%2018%2017.3999%2018H15.0495C13.567%2018%2012.2907%2019.0464%2012%2020.5V5.5C12.9443%204.24097%2014%203.5%2016.0001%203.5Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	more: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M21%2012C21%2011.1716%2020.3284%2010.5%2019.5%2010.5C18.6716%2010.5%2018%2011.1716%2018%2012C18%2012.8284%2018.6716%2013.5%2019.5%2013.5C20.3284%2013.5%2021%2012.8284%2021%2012Z%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M13.5%2012C13.5%2011.1716%2012.8284%2010.5%2012%2010.5C11.1716%2010.5%2010.5%2011.1716%2010.5%2012C10.5%2012.8284%2011.1716%2013.5%2012%2013.5C12.8284%2013.5%2013.5%2012.8284%2013.5%2012Z%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M6%2012C6%2011.1716%205.32843%2010.5%204.5%2010.5C3.67157%2010.5%203%2011.1716%203%2012C3%2012.8284%203.67157%2013.5%204.5%2013.5C5.32843%2013.5%206%2012.8284%206%2012Z%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	nextWeek: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M16%202V6M8%202V6%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M13%204H11C7.22876%204%205.34315%204%204.17157%205.17157C3%206.34315%203%208.22876%203%2012V14C3%2017.7712%203%2019.6569%204.17157%2020.8284C5.34315%2022%207.22876%2022%2011%2022H13C16.7712%2022%2018.6569%2022%2019.8284%2020.8284C21%2019.6569%2021%2017.7712%2021%2014V12C21%208.22876%2021%206.34315%2019.8284%205.17157C18.6569%204%2016.7712%204%2013%204Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3%2010H21%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M11%2014H16M8%2014H8.00898M13%2018H8M16%2018H15.991%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	plus: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M12%204V20M20%2012H4%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	project: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M8%207H16.75C18.8567%207%2019.91%207%2020.6667%207.50559C20.9943%207.72447%2021.2755%208.00572%2021.4944%208.33329C22%209.08996%2022%2010.1433%2022%2012.25C22%2015.7612%2022%2017.5167%2021.1573%2018.7779C20.7926%2019.3238%2020.3238%2019.7926%2019.7779%2020.1573C18.5167%2021%2016.7612%2021%2013.25%2021H12C7.28595%2021%204.92893%2021%203.46447%2019.5355C2%2018.0711%202%2015.714%202%2011V7.94427C2%206.1278%202%205.21956%202.38032%204.53806C2.65142%204.05227%203.05227%203.65142%203.53806%203.38032C4.21956%203%205.1278%203%206.94427%203C8.10802%203%208.6899%203%209.19926%203.19101C10.3622%203.62712%2010.8418%204.68358%2011.3666%205.73313L12%207%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	settings: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M3.99963%205.00055L9.99963%205.00031%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M12.9996%205.00031L19.9996%205.00031%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M15.9996%209.00031L15.9996%2015.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M9.99963%202.00031L9.99963%208.00031%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M11.9996%2016.0003L11.9996%2022.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M15.9996%2012.0001L19.9996%2012.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3.99963%2012.0005L12.9996%2012.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M11.9996%2019.0003L19.9996%2019.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3.99963%2019.0005L8.99963%2019.0003%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	sidebar: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M2%2012C2%208.31087%202%206.4663%202.81382%205.15877C3.1149%204.67502%203.48891%204.25427%203.91891%203.91554C5.08116%203%206.72077%203%2010%203H14C17.2792%203%2018.9188%203%2020.0811%203.91554C20.5111%204.25427%2020.8851%204.67502%2021.1862%205.15877C22%206.4663%2022%208.31087%2022%2012C22%2015.6891%2022%2017.5337%2021.1862%2018.8412C20.8851%2019.325%2020.5111%2019.7457%2020.0811%2020.0845C18.9188%2021%2017.2792%2021%2014%2021H10C6.72077%2021%205.08116%2021%203.91891%2020.0845C3.48891%2019.7457%203.1149%2019.325%202.81382%2018.8412C2%2017.5337%202%2015.6891%202%2012Z%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M9.5%203L9.5%2021%22%20stroke%3D%22currentColor%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M5%207H6M5%2010H6%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	someday: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%2212%22%20r%3D%2210%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M12%208V12L14%2014%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	statistics: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M7%2017L7%2013%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M12%2017L12%207%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M17%2017L17%2011%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M2.5%2012C2.5%207.52166%202.5%205.28249%203.89124%203.89124C5.28249%202.5%207.52166%202.5%2012%202.5C16.4783%202.5%2018.7175%202.5%2020.1088%203.89124C21.5%205.28249%2021.5%207.52166%2021.5%2012C21.5%2016.4783%2021.5%2018.7175%2020.1088%2020.1088C18.7175%2021.5%2016.4783%2021.5%2012%2021.5C7.52166%2021.5%205.28249%2021.5%203.89124%2020.1088C2.5%2018.7175%202.5%2016.4783%202.5%2012Z%22%20stroke%3D%22currentColor%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	trash: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M19.5%205.5L18.8803%2015.5251C18.7219%2018.0864%2018.6428%2019.3671%2018.0008%2020.2879C17.6833%2020.7431%2017.2747%2021.1273%2016.8007%2021.416C15.8421%2022%2014.559%2022%2011.9927%2022C9.42312%2022%208.1383%2022%207.17905%2021.4149C6.7048%2021.1257%206.296%2020.7408%205.97868%2020.2848C5.33688%2019.3626%205.25945%2018.0801%205.10461%2015.5152L4.5%205.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3%205.5H21M16.0557%205.5L15.3731%204.09173C14.9196%203.15626%2014.6928%202.68852%2014.3017%202.39681C14.215%202.3321%2014.1231%202.27454%2014.027%202.2247C13.5939%202%2013.0741%202%2012.0345%202C10.9688%202%2010.436%202%209.99568%202.23412C9.8981%202.28601%209.80498%202.3459%209.71729%202.41317C9.32164%202.7167%209.10063%203.20155%208.65861%204.17126L8.05292%205.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M9.5%2016.5L9.5%2010.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M14.5%2016.5L14.5%2010.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	week: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M16%202V6M8%202V6%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M13%204H11C7.22876%204%205.34315%204%204.17157%205.17157C3%206.34315%203%208.22876%203%2012V14C3%2017.7712%203%2019.6569%204.17157%2020.8284C5.34315%2022%207.22876%2022%2011%2022H13C16.7712%2022%2018.6569%2022%2019.8284%2020.8284C21%2019.6569%2021%2017.7712%2021%2014V12C21%208.22876%2021%206.34315%2019.8284%205.17157C18.6569%204%2016.7712%204%2013%204Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M3%2010H21%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M10%2018.5002L9.99999%2013.8474C9.99999%2013.6557%209.86325%2013.5002%209.69458%2013.5002H9M14%2018.4983L15.4855%2013.8923C15.4951%2013.8626%2015.5%2013.8315%2015.5%2013.8002C15.5%2013.6346%2015.3657%2013.5002%2015.2%2013.5002L13%2013.5%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E",
	display: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M4%2011L4%2021%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M19%2013L19%2021%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M19%203L19%207%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M11.5%203L11.5%2013%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M4%203L4%205%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M11.5%2019L11.5%2021%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M2%209.5C2%209.03406%202%208.80109%202.07612%208.61732C2.17761%208.37229%202.37229%208.17761%202.61732%208.07612C2.80109%208%203.03406%208%203.5%208H4.5C4.96594%208%205.19891%208%205.38268%208.07612C5.62771%208.17761%205.82239%208.37229%205.92388%208.61732C6%208.80109%206%209.03406%206%209.5C6%209.96594%206%2010.1989%205.92388%2010.3827C5.82239%2010.6277%205.62771%2010.8224%205.38268%2010.9239C5.19891%2011%204.96594%2011%204.5%2011H3.5C3.03406%2011%202.80109%2011%202.61732%2010.9239C2.37229%2010.8224%202.17761%2010.6277%202.07612%2010.3827C2%2010.1989%202%209.96594%202%209.5Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M17%2011.5C17%2011.0341%2017%2010.8011%2017.0761%2010.6173C17.1776%2010.3723%2017.3723%2010.1776%2017.6173%2010.0761C17.8011%2010%2018.0341%2010%2018.5%2010H19.5C19.9659%2010%2020.1989%2010%2020.3827%2010.0761C20.6277%2010.1776%2020.8224%2010.3723%2020.9239%2010.6173C21%2010.8011%2021%2011.0341%2021%2011.5C21%2011.9659%2021%2012.1989%2020.9239%2012.3827C20.8224%2012.6277%2020.6277%2012.8224%2020.3827%2012.9239C20.1989%2013%2019.9659%2013%2019.5%2013H18.5C18.0341%2013%2017.8011%2013%2017.6173%2012.9239C17.3723%2012.8224%2017.1776%2012.6277%2017.0761%2012.3827C17%2012.1989%2017%2011.9659%2017%2011.5Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3Cpath%20d%3D%22M9.5%2014.5C9.5%2014.0341%209.5%2013.8011%209.57612%2013.6173C9.67761%2013.3723%209.87229%2013.1776%2010.1173%2013.0761C10.3011%2013%2010.5341%2013%2011%2013H12C12.4659%2013%2012.6989%2013%2012.8827%2013.0761C13.1277%2013.1776%2013.3224%2013.3723%2013.4239%2013.6173C13.5%2013.8011%2013.5%2014.0341%2013.5%2014.5C13.5%2014.9659%2013.5%2015.1989%2013.4239%2015.3827C13.3224%2015.6277%2013.1277%2015.8224%2012.8827%2015.9239C12.6989%2016%2012.4659%2016%2012%2016H11C10.5341%2016%2010.3011%2016%2010.1173%2015.9239C9.87229%2015.8224%209.67761%2015.6277%209.57612%2015.3827C9.5%2015.1989%209.5%2014.9659%209.5%2014.5Z%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E%0A",
	windowMin: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22M6%2012h12%22%20stroke%3D%22currentColor%22%20stroke-width%3D%222%22%20stroke-linecap%3D%22round%22%2F%3E%3C%2Fsvg%3E%0A",
	windowMax: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Crect%20x%3D%226.5%22%20y%3D%226.5%22%20width%3D%2211%22%20height%3D%2211%22%20rx%3D%221.5%22%20stroke%3D%22currentColor%22%20stroke-width%3D%222%22%2F%3E%3C%2Fsvg%3E%0A",
	windowClose: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%3E%3Cpath%20d%3D%22m6%206%2012%2012M18%206%206%2018%22%20stroke%3D%22currentColor%22%20stroke-linecap%3D%22round%22%20stroke-width%3D%221.5%22%2F%3E%3C%2Fsvg%3E%0A",
	server: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%2212%22%20r%3D%222.6%22%2F%3E%3Cpath%20d%3D%22M12%209.4c0-3.2%201.1-5.9%202.8-5.9%201.6%200%202.3%202.5.9%205.1%22%2F%3E%3Cpath%20d%3D%22M14.6%2012c3.2%200%205.9%201.1%205.9%202.8%200%201.6-2.5%202.3-5.1.9%22%2F%3E%3Cpath%20d%3D%22M12%2014.6c0%203.2-1.1%205.9-2.8%205.9-1.6%200-2.3-2.5-.9-5.1%22%2F%3E%3Cpath%20d%3D%22M9.4%2012c-3.2%200-5.9-1.1-5.9-2.8%200-1.6%202.5-2.3%205.1-.9%22%2F%3E%3C%2Fsvg%3E",
	info: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%2212%22%20r%3D%229%22%2F%3E%3Cpath%20d%3D%22M12%2011v6%22%2F%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%227.6%22%20r%3D%22.95%22%20fill%3D%22currentColor%22%20stroke%3D%22none%22%2F%3E%3C%2Fsvg%3E",
	palette: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M12%203a9%209%200%201%200%200%2018h1.5a2.1%202.1%200%200%200%200-4.2h-.6a1.9%201.9%200%200%201%200-3.8H16a5%205%200%200%200%205-5C21%205.3%2017%203%2012%203Z%22%2F%3E%3Ccircle%20cx%3D%227.8%22%20cy%3D%2210%22%20r%3D%221.1%22%20fill%3D%22currentColor%22%20stroke%3D%22none%22%2F%3E%3Ccircle%20cx%3D%2210.2%22%20cy%3D%226.8%22%20r%3D%221.1%22%20fill%3D%22currentColor%22%20stroke%3D%22none%22%2F%3E%3Ccircle%20cx%3D%2214.4%22%20cy%3D%226.6%22%20r%3D%221.1%22%20fill%3D%22currentColor%22%20stroke%3D%22none%22%2F%3E%3C%2Fsvg%3E",
	apps: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Crect%20x%3D%223.5%22%20y%3D%223.5%22%20width%3D%227%22%20height%3D%227%22%20rx%3D%222.2%22%2F%3E%3Crect%20x%3D%2213.5%22%20y%3D%223.5%22%20width%3D%227%22%20height%3D%227%22%20rx%3D%222.2%22%2F%3E%3Crect%20x%3D%223.5%22%20y%3D%2213.5%22%20width%3D%227%22%20height%3D%227%22%20rx%3D%222.2%22%2F%3E%3Crect%20x%3D%2213.5%22%20y%3D%2213.5%22%20width%3D%227%22%20height%3D%227%22%20rx%3D%222.2%22%2F%3E%3C%2Fsvg%3E",
	plug: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M9%203v5M15%203v5%22%2F%3E%3Cpath%20d%3D%22M6%208h12v3a6%206%200%200%201-6%206%206%206%200%200%201-6-6V8Z%22%2F%3E%3Cpath%20d%3D%22M12%2017v4%22%2F%3E%3C%2Fsvg%3E",
	database: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cellipse%20cx%3D%2212%22%20cy%3D%226%22%20rx%3D%227.5%22%20ry%3D%223%22%2F%3E%3Cpath%20d%3D%22M4.5%206v6c0%201.66%203.36%203%207.5%203s7.5-1.34%207.5-3V6%22%2F%3E%3Cpath%20d%3D%22M4.5%2012v6c0%201.66%203.36%203%207.5%203s7.5-1.34%207.5-3v-6%22%2F%3E%3C%2Fsvg%3E",
	pulse: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M2.5%2012h4l2-5%203%2010%202.5-6%201.5%203h6%22%2F%3E%3C%2Fsvg%3E",
	monitor: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Crect%20x%3D%222.5%22%20y%3D%224%22%20width%3D%2219%22%20height%3D%2212.5%22%20rx%3D%222.6%22%2F%3E%3Cpath%20d%3D%22M9%2020.5h6M12%2016.5v4%22%2F%3E%3C%2Fsvg%3E",
	globe: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%2212%22%20r%3D%229%22%2F%3E%3Cpath%20d%3D%22M3.2%209h17.6M3.2%2015h17.6%22%2F%3E%3Cpath%20d%3D%22M12%203a14%2014%200%200%201%200%2018a14%2014%200%200%201%200-18Z%22%2F%3E%3C%2Fsvg%3E",
	key: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%228.2%22%20cy%3D%228.2%22%20r%3D%224.4%22%2F%3E%3Cpath%20d%3D%22M11.4%2011.4%2020%2020M17%2017l-2%202M14%2014l-2%202%22%2F%3E%3C%2Fsvg%3E",
	chip: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Crect%20x%3D%226.5%22%20y%3D%226.5%22%20width%3D%2211%22%20height%3D%2211%22%20rx%3D%222.6%22%2F%3E%3Cpath%20d%3D%22M10%203v3.5M14%203v3.5M10%2017.5V21M14%2017.5V21M3%2010h3.5M3%2014h3.5M17.5%2010H21M17.5%2014H21%22%2F%3E%3C%2Fsvg%3E",
	gear: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%2212%22%20cy%3D%2212%22%20r%3D%223.1%22%2F%3E%3Cpath%20d%3D%22M19.4%2013.4a7.6%207.6%200%200%200%200-2.8l1.9-1.4-2-3.4-2.2%201a7.6%207.6%200%200%200-2.4-1.4L14.3%202.2H9.7l-.4%202.9a7.6%207.6%200%200%200-2.4%201.4l-2.2-1-2%203.4%201.9%201.4a7.6%207.6%200%200%200%200%202.8l-1.9%201.4%202%203.4%202.2-1a7.6%207.6%200%200%200%202.4%201.4l.4%202.9h4.6l.4-2.9a7.6%207.6%200%200%200%202.4-1.4l2.2%201%202-3.4-1.9-1.4Z%22%2F%3E%3C%2Fsvg%3E",
	code: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M9%208.5%205%2012l4%203.5M15%208.5%2019%2012l-4%203.5%22%2F%3E%3C%2Fsvg%3E",
	search: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Ccircle%20cx%3D%2211%22%20cy%3D%2211%22%20r%3D%226.5%22%2F%3E%3Cpath%20d%3D%22M16%2016l4.4%204.4%22%2F%3E%3C%2Fsvg%3E",
	power: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M12%203v8%22%2F%3E%3Cpath%20d%3D%22M7.6%206.3a7.6%207.6%200%201%200%208.8%200%22%2F%3E%3C%2Fsvg%3E",
	chevron: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M9.5%206l6%206-6%206%22%2F%3E%3C%2Fsvg%3E",
	refresh: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M20%2011a8%208%200%201%200-2.7%207%22%2F%3E%3Cpath%20d%3D%22M20%204.6V11h-6.4%22%2F%3E%3C%2Fsvg%3E",
	shield: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M12%203%205%206v5.6c0%204.3%203%208%207%209.4%204-1.4%207-5.1%207-9.4V6l-7-3Z%22%2F%3E%3C%2Fsvg%3E",
	external: "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M13.5%204.5H20V11%22%2F%3E%3Cpath%20d%3D%22M20%204.5%2011.5%2013%22%2F%3E%3Cpath%20d%3D%22M18%2014v3.5a2.5%202.5%200%200%201-2.5%202.5H6a2.5%202.5%200%200%201-2.5-2.5V9A2.5%202.5%200%200%201%206%206.5h3.5%22%2F%3E%3C%2Fsvg%3E"
};

//#endregion
//#region node_modules/@makekosmos/imago-nsh/crates/imago-nsh/ui/components.tsx
function Icon(props) {
	return (() => {
		const __nsh_hv0 = element("image");
		bindProp(__nsh_hv0, "src", () => icons[props.name]);
		bindProp(__nsh_hv0, "style", () => ({
			width: props.size ?? 16,
			height: props.size ?? 16,
			tintColor: props.color
		}));
		return __nsh_hv0;
	})();
}

//#endregion
//#region node_modules/@makekosmos/imago-nsh/crates/imago-nsh/ui/palettes.ts
/**
* Cobalt Foundry: forged steel greys lit by a cobalt arc. The Manager wears it
* as its signature scheme; every consumer may ship its own palettes next to
* this one, and the shape is the same so a choice can travel between apps.
*/
const cobaltFoundry = {
	background: "#090c15",
	shell: "#0d1019",
	card: "#101318",
	raised: "#181c25",
	input: "#1e222b",
	border: "#1e222b",
	foreground: "#ffffff",
	muted: "#717580",
	faint: "#8f9498",
	accent: "#1784fc",
	accentText: "#101318",
	danger: "#e67b71",
	dangerFill: "#3e1f24",
	success: "#3fbf7f"
};
/** The same cobalt palette for a light room: pale steel, same arc. */
const cobaltFoundryLight = {
	background: "#f7f8fa",
	shell: "#eff1f5",
	card: "#ffffff",
	raised: "#eef1f6",
	border: "#dde2ea",
	foreground: "#111820",
	muted: "#5f6a7a",
	faint: "#8c96a6",
	accent: "#1f6df5",
	accentText: "#ffffff",
	danger: "#d5373c",
	dangerFill: "#fdecec",
	success: "#1f8a5b"
};
function palette(key, name, colors) {
	return {
		key,
		name,
		colors
	};
}
const lightThemes = [
	palette("merino-light", "Merino Light", {
		...cobaltFoundryLight,
		background: "#f5f3ef",
		shell: "#eeeae4",
		card: "#ffffff",
		raised: "#f1ede7",
		border: "#e0dad1",
		foreground: "#221f1b",
		muted: "#6b645b",
		faint: "#948c82",
		accent: "#2563eb",
		accentText: "#ffffff"
	}),
	palette("alpine-milk", "Alpine Milk", {
		...cobaltFoundryLight,
		background: "#f7f9fb",
		shell: "#eef2f7",
		card: "#ffffff",
		raised: "#eaf0f7",
		border: "#dbe3ee",
		foreground: "#152030",
		muted: "#5b6a7d",
		faint: "#8794a8",
		accent: "#0f7bd4"
	}),
	palette("buttercream-diner", "Buttercream Diner", {
		...cobaltFoundryLight,
		background: "#fdf8ea",
		shell: "#f8f0dc",
		card: "#fffdf4",
		raised: "#f7eed8",
		border: "#eadfc2",
		foreground: "#2a2113",
		muted: "#736450",
		faint: "#9c8f7c",
		accent: "#d1622a",
		danger: "#c2402f"
	}),
	palette("coastal-linen", "Coastal Linen", {
		...cobaltFoundryLight,
		background: "#f3f8f7",
		shell: "#e9f2f1",
		card: "#ffffff",
		raised: "#e8f1f0",
		border: "#d5e4e2",
		foreground: "#132524",
		muted: "#566a69",
		faint: "#839796",
		accent: "#0f9aa8",
		success: "#149a72"
	})
];
const darkThemes = [
	palette("cinder-peak", "Cinder Peak", {
		...cobaltFoundry,
		background: "#121010",
		shell: "#161313",
		card: "#1c1818",
		raised: "#241f1f",
		border: "#2e2727",
		foreground: "#f1eae7",
		muted: "#a2968f",
		faint: "#736762",
		accent: "#f0743f",
		danger: "#e5544b",
		dangerFill: "#2b1614"
	}),
	palette("cobalt-foundry", "Cobalt Foundry", cobaltFoundry),
	palette("ember-hollow", "Ember Hollow", {
		...cobaltFoundry,
		background: "#0e0c09",
		shell: "#12100b",
		card: "#191509",
		raised: "#221c10",
		border: "#2d2515",
		foreground: "#f4ead6",
		muted: "#a6987c",
		faint: "#776b53",
		accent: "#f2a03d",
		danger: "#e2603f",
		dangerFill: "#2a1a10",
		success: "#9cbf4b"
	}),
	palette("heliotrope", "Heliotrope", {
		...cobaltFoundry,
		background: "#0c0a12",
		shell: "#100d17",
		card: "#16121f",
		raised: "#1e1929",
		border: "#291f38",
		foreground: "#ece7f8",
		muted: "#9a90b4",
		faint: "#6d6488",
		accent: "#b07cff",
		danger: "#ef5d8b",
		dangerFill: "#291322"
	})
];

//#endregion
//#region node_modules/@makekosmos/imago-nsh/crates/imago-nsh/ui/settings.tsx
/** One hex colour nudged toward another: the sheen and the shade of a tile. */
function mix(hex, toward, amount) {
	const parse = (value) => {
		const clean = value.replace("#", "");
		return [
			0,
			2,
			4
		].map((index) => parseInt(clean.slice(index, index + 2), 16));
	};
	const from = parse(hex);
	const to = parse(toward);
	const channel = (index) => {
		const value = Math.round(from[index] + (to[index] - from[index]) * amount);
		return Math.max(0, Math.min(255, value)).toString(16).padStart(2, "0");
	};
	return `#${[
		0,
		1,
		2
	].map(channel).join("")}`;
}
/** Piecewise interpolation across a colour list. */
function along(colours, t) {
	const span = colours.length - 1;
	const at = Math.min(span - 1, Math.floor(t * span));
	const local = t * span - at;
	return mix(colours[at], colours[at + 1], local);
}
/**
* A tile's fill. The design paints a white sheen over an accent pair and shades
* the foot; NSH paints one background image, so the same three layers are
* sampled into stops here — thirteen of them, which is what a 24pt tile needs
* for the gloss to read as a gloss.
*/
function tileGradient(accent, accentTo) {
	const colours = accentTo === "spectrum" ? [
		"#f79411",
		"#13c370",
		"#1784fc",
		"#915bfc",
		"#fb3499"
	] : [accent, accentTo ?? accent];
	return `linear-gradient(180deg, ${Array.from({ length: 13 }, (_, index) => {
		const t = index / 12;
		const base = along(colours, t);
		const sheen = t <= .5 ? .23 * (1 - t / .5) : -.17 * ((t - .5) / .5);
		return `${sheen >= 0 ? mix(base, "#ffffff", sheen) : mix(base, "#000000", -sheen)} ${Math.round(t * 100)}%`;
	}).join(", ")})`;
}
/** The Mundus mark in a pill of its own — the running server's avatar. */
function LogoTile(props) {
	const size = props.size ?? settingsMetrics.logoTileSize;
	return (() => {
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "class", "items-center justify-center");
		prop(__nsh_hv1, "style", {
			width: size,
			height: size,
			borderRadius: size / 2,
			backgroundImage: "linear-gradient(135deg, #93aeea 0%, #5b83d6 34%, #359ecb 100%)",
			borderWidth: 1,
			borderColor: "rgba(255,255,255,0.22)"
		});
		insert(__nsh_hv1, component(Icon, {
			name: "mundus",
			get size() {
				return Math.round(size * .56);
			},
			color: "#ffffff"
		}));
		return __nsh_hv1;
	})();
}
function SearchField(props) {
	return (() => {
		const __nsh_hv7 = element("view");
		prop(__nsh_hv7, "class", "w-full flex-row items-center gap-2 rounded-[10px] px-2.5");
		bindProp(__nsh_hv7, "style", () => ({
			height: settingsMetrics.searchHeight,
			backgroundColor: props.colors.input ?? props.colors.raised
		}));
		const __nsh_hv8 = element("input");
		bindProp(__nsh_hv8, "value", () => props.value);
		prop(__nsh_hv8, "onChange", (event) => props.onChange(event.value));
		bindProp(__nsh_hv8, "placeholder", () => props.placeholder);
		bindProp(__nsh_hv8, "accessibilityLabel", () => props.placeholder);
		prop(__nsh_hv8, "class", "flex-1 text-[13px]");
		bindProp(__nsh_hv8, "style", () => ({
			backgroundColor: "transparent",
			color: props.colors.foreground
		}));
		append(__nsh_hv7, __nsh_hv8);
		insert(__nsh_hv7, component(Icon, {
			name: "search",
			size: 16,
			get color() {
				return props.colors.faint;
			}
		}), __nsh_hv8);
		return __nsh_hv7;
	})();
}
function SectionLabel(props) {
	return (() => {
		const __nsh_hv20 = element("view");
		prop(__nsh_hv20, "class", "flex-row items-center justify-between");
		bindProp(__nsh_hv20, "style", () => ({
			marginBottom: settingsMetrics.labelGap,
			paddingLeft: settingsMetrics.cardPaddingX
		}));
		const __nsh_hv21 = element("text");
		bindProp(__nsh_hv21, "style", () => ({
			fontSize: 13,
			lineHeight: 18,
			fontWeight: "500",
			color: props.colors.foreground
		}));
		insert(__nsh_hv21, () => props.children);
		append(__nsh_hv20, __nsh_hv21);
		insert(__nsh_hv20, () => props.right ?? null);
		return __nsh_hv20;
	})();
}
function Card(props) {
	return (() => {
		const __nsh_hv22 = element("view");
		prop(__nsh_hv22, "class", "overflow-hidden");
		bindProp(__nsh_hv22, "style", () => ({
			backgroundColor: props.colors.card,
			borderRadius: settingsMetrics.cardRadius,
			padding: props.padded ? 12 : 0
		}));
		insert(__nsh_hv22, () => props.children);
		return __nsh_hv22;
	})();
}
/** One card row: optional avatar, title over an optional subtitle. */
function CardRow(props) {
	return (() => {
		const __nsh_hv23 = element("view");
		bindProp(__nsh_hv23, "onPress", () => props.onPress);
		bindProp(__nsh_hv23, "role", () => props.onPress ? "button" : void 0);
		prop(__nsh_hv23, "class", "flex-row items-center gap-2.5");
		bindProp(__nsh_hv23, "style", () => ({
			height: props.logo || props.icon ? settingsMetrics.cardRowHeight : void 0,
			minHeight: settingsMetrics.cardDenseRowHeight,
			paddingLeft: settingsMetrics.cardPaddingX,
			paddingRight: settingsMetrics.cardPaddingX,
			paddingTop: props.logo || props.icon ? 0 : settingsMetrics.cardDensePaddingY,
			paddingBottom: props.logo || props.icon ? 0 : settingsMetrics.cardDensePaddingY,
			borderBottomWidth: props.last ? 0 : 1,
			borderColor: props.colors.border
		}));
		const __nsh_hv25 = element("view");
		prop(__nsh_hv25, "class", "flex-1");
		prop(__nsh_hv25, "style", { gap: 4 });
		const __nsh_hv26 = element("text");
		bindProp(__nsh_hv26, "style", () => ({
			fontSize: 13,
			lineHeight: 18,
			fontWeight: "600",
			color: props.colors.foreground
		}));
		insert(__nsh_hv26, () => props.title);
		append(__nsh_hv25, __nsh_hv26);
		insert(__nsh_hv25, () => props.subtitle ? (() => {
			const __nsh_hv27 = element("text");
			bindProp(__nsh_hv27, "style", () => ({
				fontSize: 12,
				lineHeight: 16,
				fontWeight: "500",
				color: props.colors.muted
			}));
			insert(__nsh_hv27, () => props.subtitle);
			return __nsh_hv27;
		})() : null);
		append(__nsh_hv23, __nsh_hv25);
		insert(__nsh_hv23, () => props.logo ? component(LogoTile, {}) : props.icon ? (() => {
			const __nsh_hv24 = element("view");
			prop(__nsh_hv24, "class", "items-center justify-center");
			bindProp(__nsh_hv24, "style", () => ({
				width: 35,
				height: 35,
				borderRadius: 18,
				backgroundColor: props.iconFill,
				backgroundImage: props.iconFill ? void 0 : props.accent ? tileGradient(props.accent, props.accentTo) : `linear-gradient(-135deg, #151515 0%, #303030 100%)`,
				borderWidth: 1,
				borderColor: "#00000033"
			}));
			insert(__nsh_hv24, component(Icon, {
				get name() {
					return props.icon;
				},
				size: 20,
				get color() {
					return props.accent ? "#ffffff" : "#bfbfbf";
				}
			}));
			return __nsh_hv24;
		})() : null, __nsh_hv25);
		insert(__nsh_hv23, () => props.right ?? null);
		return __nsh_hv23;
	})();
}
/** Label on the left, control on the right — the "Название" row of settings. */
function FieldRow(props) {
	return (() => {
		const __nsh_hv28 = element("view");
		prop(__nsh_hv28, "class", "flex-row items-center");
		bindProp(__nsh_hv28, "style", () => ({
			height: settingsMetrics.fieldRowHeight,
			gap: 40,
			paddingLeft: settingsMetrics.cardPaddingX,
			paddingRight: settingsMetrics.cardPaddingX,
			paddingTop: settingsMetrics.cardPaddingY,
			paddingBottom: settingsMetrics.cardPaddingY,
			borderBottomWidth: props.last ? 0 : 1,
			borderColor: props.colors.border
		}));
		const __nsh_hv29 = element("text");
		bindProp(__nsh_hv29, "style", () => ({
			width: props.labelWidth ?? settingsMetrics.fieldLabelWidth,
			fontSize: 13,
			fontWeight: "500",
			color: props.colors.foreground
		}));
		insert(__nsh_hv29, () => props.label);
		const __nsh_hv30 = element("view");
		prop(__nsh_hv30, "class", "flex-1");
		insert(__nsh_hv30, () => props.children);
		append(__nsh_hv28, __nsh_hv29);
		append(__nsh_hv28, __nsh_hv30);
		return __nsh_hv28;
	})();
}
function TextInput(props) {
	return (() => {
		const __nsh_hv31 = element("input");
		bindProp(__nsh_hv31, "value", () => props.value);
		prop(__nsh_hv31, "onChange", (event) => props.onChange(event.value));
		bindProp(__nsh_hv31, "onSubmit", () => props.onSubmit ? (event) => props.onSubmit?.(event.text) : void 0);
		bindProp(__nsh_hv31, "placeholder", () => props.placeholder);
		bindProp(__nsh_hv31, "accessibilityLabel", () => props.accessibilityLabel ?? props.placeholder);
		prop(__nsh_hv31, "class", "h-8 rounded-[10px] px-2.5 text-[13px]");
		bindProp(__nsh_hv31, "style", () => ({
			backgroundColor: props.colors.input ?? props.colors.raised,
			color: props.colors.foreground
		}));
		return __nsh_hv31;
	})();
}
function PillButton(props) {
	const tone = props.tone ?? "neutral";
	const background = tone === "accent" ? props.colors.accent : tone === "danger" ? props.colors.dangerFill : props.colors.raised;
	const color = tone === "accent" ? props.colors.accentText : tone === "danger" ? props.colors.danger : props.colors.foreground;
	return (() => {
		const __nsh_hv32 = element("view");
		bindProp(__nsh_hv32, "onPress", () => props.disabled ? void 0 : props.onPress);
		prop(__nsh_hv32, "role", "button");
		bindProp(__nsh_hv32, "accessibilityLabel", () => props.label);
		bindProp(__nsh_hv32, "accessibilityState", () => ({ disabled: !!props.disabled }));
		prop(__nsh_hv32, "hoverStyle", { opacity: .9 });
		prop(__nsh_hv32, "class", "items-center justify-center rounded-full");
		bindProp(__nsh_hv32, "style", () => ({
			paddingLeft: 8,
			paddingRight: 8,
			paddingTop: 4,
			paddingBottom: 4,
			backgroundColor: background,
			opacity: props.disabled ? .5 : 1
		}));
		const __nsh_hv33 = element("text");
		prop(__nsh_hv33, "style", {
			fontSize: 13,
			lineHeight: 15,
			fontWeight: "500",
			color
		});
		insert(__nsh_hv33, () => props.label);
		append(__nsh_hv32, __nsh_hv33);
		return __nsh_hv32;
	})();
}
function Switch(props) {
	const off = !props.value;
	return (() => {
		const __nsh_hv34 = element("view");
		prop(__nsh_hv34, "onPress", () => props.onChange(!props.value));
		prop(__nsh_hv34, "role", "switch");
		bindProp(__nsh_hv34, "accessibilityLabel", () => props.accessibilityLabel);
		bindProp(__nsh_hv34, "accessibilityState", () => ({ checked: props.value }));
		prop(__nsh_hv34, "class", "justify-center");
		bindProp(__nsh_hv34, "style", () => ({
			width: 44,
			height: 22,
			marginTop: 1,
			borderRadius: 11,
			backgroundColor: props.value ? props.colors.accent : props.colors.input ?? props.colors.raised,
			borderWidth: off ? 1 : 0,
			borderColor: "#2f3543"
		}));
		const __nsh_hv35 = element("view");
		bindProp(__nsh_hv35, "style", () => ({
			position: "absolute",
			width: 24,
			height: 18,
			borderRadius: 9,
			left: props.value ? 18 : 1,
			top: 1,
			backgroundColor: "#ffffff",
			boxShadow: "0 1px 1.75px #00000038"
		}));
		append(__nsh_hv34, __nsh_hv35);
		insert(__nsh_hv34, () => off ? (() => {
			const __nsh_hv36 = element("view");
			prop(__nsh_hv36, "style", {
				position: "absolute",
				width: 6,
				height: 6,
				borderRadius: 3,
				borderWidth: .6,
				borderColor: "#ffffff",
				opacity: .75,
				left: 32,
				top: 7
			});
			return __nsh_hv36;
		})() : null);
		return __nsh_hv34;
	})();
}
function StatusDot(props) {
	const tone = props.tone ?? "success";
	const color = tone === "success" ? props.colors.success : tone === "danger" ? props.colors.danger : props.colors.muted;
	return (() => {
		const __nsh_hv37 = element("view");
		prop(__nsh_hv37, "style", {
			width: 7,
			height: 7,
			borderRadius: 4,
			backgroundColor: color
		});
		return __nsh_hv37;
	})();
}

//#endregion
//#region navigation.ts
/**
* Sidebar groups of the Manager. The first group answers "is my Mundus well",
* the second covers what runs inside it, the third the machines and their data,
* and the last two the app's own configuration.
*/
const navigation = [
	{
		id: "server",
		entries: [{
			route: "server",
			label: "Сервер",
			icon: "server",
			accent: "#3b82f6",
			accentTo: "#359ecb"
		}]
	},
	{
		id: "running",
		entries: [
			{
				route: "plugins",
				label: "Плагины",
				icon: "apps",
				accent: "#fa6710",
				accentTo: "#fc2d3e"
			},
			{
				route: "integrations",
				label: "Интеграции",
				icon: "plug",
				accent: "#915bfc",
				accentTo: "#fb3499"
			},
			{
				route: "storage",
				label: "Хранилище",
				icon: "database",
				accent: "#15b8a4",
				accentTo: "#1784fc"
			},
			{
				route: "activity",
				label: "Активность",
				icon: "pulse",
				accent: "#fa6710",
				accentTo: "#fc2d3e"
			}
		]
	},
	{
		id: "machines",
		entries: [{
			route: "devices",
			label: "Девайсы",
			icon: "monitor",
			accent: "#1784fc",
			accentTo: "#15b8a4"
		}, {
			route: "browser",
			label: "Браузер",
			icon: "globe",
			accent: "#915bfc",
			accentTo: "#1784fc"
		}]
	},
	{
		id: "data",
		entries: [{
			route: "keys",
			label: "Ключи",
			icon: "key",
			accent: "#f79411",
			accentTo: "#fa6710"
		}, {
			route: "models",
			label: "Модели",
			icon: "chip",
			accent: "#fb3499",
			accentTo: "#915bfc"
		}]
	},
	{
		id: "config",
		entries: [{
			route: "settings",
			label: "Настройки",
			icon: "gear",
			accent: "#777777",
			accentTo: "#777777"
		}]
	}
];

//#endregion
//#region history.ts
/**
* Where the pages have been, so the header's anchors can walk back and forth.
*
* The sidebar and the Engine's `navigate` event both land here: a page reached
* from either place is a step the anchors can undo. A route that equals the
* current one is not a step — pressing the sidebar row you are already on must
* not fill the history with the same page.
*/
const initial = globalThis.__MUNDUS_MANAGER_ROUTE__ ?? "server";
const stack = signal([initial]);
const at = signal(0);
const currentRoute = () => stack()[at()];
const canBack = () => at() > 0;
const canForward = () => at() < stack().length - 1;
function go(route) {
	if (route === currentRoute()) return;
	const kept = stack().slice(0, at() + 1);
	stack.set([...kept, route]);
	at.set(kept.length);
}
function back() {
	if (canBack()) at.set(at() - 1);
}
function forward() {
	if (canForward()) at.set(at() + 1);
}

//#endregion
//#region chrome.ts
/**
* The band the caption buttons sit in, which the page heading and the sidebar's
* search field share with them: the first row of the window is the titlebar.
*/
const TITLEBAR_HEIGHT = 36;
function reservesCaptions() {
	const host = platform();
	return host === "win32" || host === "windows";
}
/** The width of the strip the caption buttons take, in points. */
function captionStrip() {
	return reservesCaptions() ? 138 : 0;
}
/** Below the lights: the three buttons, then air, so the two bands read apart. */
const MAC_LIGHTS_INSET = 48;
/**
* The page header's height. macOS puts the traffic lights a shade lower than
* the band a Windows caption sits in, so the title needs a taller row to line
* up with them; the window's own titlebar is not resized.
*/
function headerHeight() {
	const host = platform();
	if (host === "darwin" || host === "macos") return 48;
	return 36;
}
/**
* The space above the sidebar's search field. Windows draws its caption
* buttons on the right, so the field shares the title band there; macOS draws
* the lights on the left, over the field, so the field starts under them.
*/
function sidebarTopInset(searchHeight) {
	const host = platform();
	if (host === "win32" || host === "windows") return (36 - searchHeight) / 2;
	if (host === "darwin" || host === "macos") return MAC_LIGHTS_INSET;
	return 16;
}

//#endregion
//#region sidebar.tsx
/**
* A colour moved toward black or white.
*
* The kit's tiles are gradient plates with a white sheen — the colour language
* of a launcher. Mundus wears one flat plate per row, a shade darker than the
* page, so the glyph carries the meaning and the sidebar stays quiet. NSH
* paints a gradient, so the plate is a single solid colour here.
*/
function shade(hex, amount, toward = "black") {
	const parse = (value) => {
		const clean = value.replace("#", "");
		return [
			0,
			2,
			4
		].map((index) => parseInt(clean.slice(index, index + 2), 16));
	};
	const from = parse(hex);
	const target = toward === "black" ? [
		0,
		0,
		0
	] : [
		255,
		255,
		255
	];
	const channel = (index) => {
		const value = Math.round(from[index] + (target[index] - from[index]) * amount);
		return Math.max(0, Math.min(255, value)).toString(16).padStart(2, "0");
	};
	return `#${[
		0,
		1,
		2
	].map(channel).join("")}`;
}
/** The sidebar's own rhythm: roomier than the kit's, which packs rows tight. */
const GAP = {
	padX: 12,
	rowPadX: 12,
	rowHeight: 36,
	logoRowHeight: 48,
	searchGap: 10,
	groupGap: 14,
	entryGap: 2,
	tileLabel: 10
};
/** The tile is smaller than the kit's: a plate, not a badge. */
const TILE = 22;
/** The plate every row's glyph sits on: the page colour, a shade deeper. */
function plateColor(colors) {
	return shade(colors.background, .22);
}
/** The Mundus mark in a pill of its own — the running server's avatar. */
function MarkTile(props) {
	const size = props.size ?? settingsMetrics.logoTileSize;
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "class", "items-center justify-center");
		prop(__nsh_hv0, "style", {
			width: size,
			height: size,
			borderRadius: size / 2,
			backgroundImage: "linear-gradient(135deg, #93aeea 0%, #5b83d6 34%, #359ecb 100%)",
			borderWidth: 1,
			borderColor: "rgba(255,255,255,0.22)"
		});
		insert(__nsh_hv0, component(Icon, {
			name: "mundus",
			get size() {
				return Math.round(size * .56);
			},
			color: "#ffffff"
		}));
		return __nsh_hv0;
	})();
}
/** One glyph on a flat plate: no sheen, no hue, the page's own colour. */
function SidebarTile(props) {
	return (() => {
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "class", "items-center justify-center");
		bindProp(__nsh_hv1, "style", () => ({
			width: TILE,
			height: TILE,
			borderRadius: 6,
			backgroundColor: plateColor(props.colors),
			borderWidth: 1,
			borderColor: "rgba(255,255,255,0.045)"
		}));
		insert(__nsh_hv1, component(Icon, {
			get name() {
				return props.icon;
			},
			get size() {
				return Math.round(TILE * .7);
			},
			get color() {
				return props.colors.foreground;
			}
		}));
		return __nsh_hv1;
	})();
}
function SidebarRow(props) {
	return (() => {
		const __nsh_hv2 = element("view");
		bindProp(__nsh_hv2, "onPress", () => props.onPress);
		prop(__nsh_hv2, "role", "button");
		bindProp(__nsh_hv2, "accessibilityLabel", () => props.label);
		bindProp(__nsh_hv2, "accessibilityState", () => ({ selected: props.selected }));
		prop(__nsh_hv2, "class", "flex-row items-center rounded-[10px]");
		bindProp(__nsh_hv2, "style", () => ({
			height: props.caption ? GAP.logoRowHeight : GAP.rowHeight,
			gap: GAP.tileLabel,
			paddingLeft: GAP.rowPadX,
			paddingRight: GAP.rowPadX,
			backgroundColor: props.selected ? props.colors.raised : "transparent"
		}));
		const __nsh_hv3 = element("view");
		bindProp(__nsh_hv3, "style", () => ({ marginTop: props.caption || props.logo ? 0 : -1 }));
		insert(__nsh_hv3, () => props.logo ? component(MarkTile, {}) : component(SidebarTile, {
			get icon() {
				return props.icon;
			},
			get colors() {
				return props.colors;
			}
		}));
		const __nsh_hv4 = element("view");
		prop(__nsh_hv4, "class", "flex-1");
		prop(__nsh_hv4, "style", { gap: 2 });
		const __nsh_hv5 = element("text");
		bindProp(__nsh_hv5, "style", () => ({
			fontSize: 14,
			fontWeight: "500",
			lineHeight: 20,
			color: props.colors.foreground
		}));
		insert(__nsh_hv5, () => props.label);
		append(__nsh_hv4, __nsh_hv5);
		insert(__nsh_hv4, () => props.caption ? (() => {
			const __nsh_hv6 = element("text");
			bindProp(__nsh_hv6, "style", () => ({
				fontSize: 12,
				lineHeight: 16,
				fontWeight: "500",
				color: props.colors.muted
			}));
			insert(__nsh_hv6, () => props.caption);
			return __nsh_hv6;
		})() : null);
		append(__nsh_hv2, __nsh_hv3);
		append(__nsh_hv2, __nsh_hv4);
		return __nsh_hv2;
	})();
}
/**
* The sidebar. Its own column, so the window can keep the search field clear of
* the traffic lights: macOS puts them over the top-left corner, and a field
* starting at the very top sits under them.
*/
function Sidebar(props) {
	const query = props.search.trim().toLowerCase();
	return (() => {
		const __nsh_hv7 = element("view");
		prop(__nsh_hv7, "class", "h-full shrink-0 flex-col");
		bindProp(__nsh_hv7, "style", () => ({
			width: settingsMetrics.sidebarWidth,
			backgroundColor: props.colors.shell,
			paddingTop: props.topInset,
			paddingBottom: settingsMetrics.sidebarTop,
			paddingLeft: GAP.padX,
			paddingRight: GAP.padX
		}));
		const __nsh_hv8 = element("view");
		prop(__nsh_hv8, "class", "min-h-0 flex-1 flex-col");
		bindProp(__nsh_hv8, "style", () => ({ gap: GAP.searchGap }));
		const __nsh_hv9 = element("scroll");
		prop(__nsh_hv9, "class", "min-h-0 flex-1");
		const __nsh_hv10 = element("view");
		bindProp(__nsh_hv10, "style", () => ({ gap: GAP.groupGap }));
		insert(__nsh_hv10, () => props.groups.map((group, index) => {
			const entries = group.entries.filter((entry) => !query || entry.label.toLowerCase().includes(query));
			if (!entries.length) return null;
			return (() => {
				const __nsh_hv11 = element("view");
				bindProp(__nsh_hv11, "key", () => group.id);
				bindProp(__nsh_hv11, "style", () => ({
					gap: GAP.entryGap,
					marginBottom: index === 0 ? 0 : 0
				}));
				insert(__nsh_hv11, () => entries.map((entry) => component(SidebarRow, {
					get key() {
						return entry.route;
					},
					get label() {
						return entry.label;
					},
					get icon() {
						return entry.icon;
					},
					get caption() {
						return entry.caption;
					},
					get logo() {
						return entry.route === "server";
					},
					get selected() {
						return props.current === entry.route;
					},
					get colors() {
						return props.colors;
					},
					onPress: () => props.onNavigate(entry.route)
				})));
				return __nsh_hv11;
			})();
		}));
		append(__nsh_hv9, __nsh_hv10);
		append(__nsh_hv8, __nsh_hv9);
		insert(__nsh_hv8, component(SearchField, {
			get value() {
				return props.search;
			},
			placeholder: "Поиск настроек",
			get colors() {
				return props.colors;
			},
			get onChange() {
				return props.onSearch;
			}
		}), __nsh_hv9);
		append(__nsh_hv7, __nsh_hv8);
		return __nsh_hv7;
	})();
}

//#endregion
//#region host.ts
/**
* The host's own window: things the pages ask for and only the shell can do.
*
* A press that drags the window is one of them, and *who* moves it depends on
* whose window it is. The dev host (`nsh dev`) owns its window — it draws the
* caption buttons, reports their strip in `titleBar().right`, hit-tests the
* `windowDrag` areas itself and lets Windows move the window from them. An
* embedding host (`manager.exe`) reports nothing reserved and swallows the
* press in its surface before Windows can act on it (`Window::start_window_move`
* is an empty default on Windows), so the press has to ask the host's own Win32
* code over the bridge.
*/
let command$1;
function connectHost(send) {
	command$1 = send;
}
/** A press here moves the window, by whichever of the two means applies. */
function dragBinding() {
	return titleBar().right > 0 ? { desktop: { windowDrag: true } } : { onPressDown: () => command$1?.("window.drag") };
}

//#endregion
//#region state.ts
/**
* The Manager's mirror of what the host pushes into the page. `manager.bridge`
* fills it from the Engine (`/v1/health`, `/v1/info`, `/v1/rpc`); without the
* host — `nsh dev`, `nsh test`, a browser — `demoState` keeps every page
* readable instead of blank.
*/
/** The combination the Manager's event tap read, handed to the page that asked
* for it: the widget shows nothing of its own while the keys are held, and the
* page stores the result as a setting. */
const hotkeyCaptured = signal("");
const demoState = {
	engine: {
		status: "running",
		version: "0.10.26",
		channel: "production",
		port: 47321
	},
	client: { name: "Этот компьютер" },
	product: { bundleId: "computer.mundus.app" },
	sync: {
		running: true,
		incoming: [],
		peers: [{
			id: "desktop",
			name: "Мастерская",
			status: "online",
			platform: "windows",
			current: false
		}, {
			id: "phone",
			name: "Pixel 9",
			status: "offline",
			platform: "android",
			current: false
		}]
	},
	models: [{
		id: "whisper-small",
		name: "Whisper Small",
		size: "466 МБ",
		downloaded: true,
		downloading: false,
		selected: true,
		percent: 0
	}, {
		id: "whisper-medium",
		name: "Whisper Medium",
		size: "1.5 ГБ",
		downloaded: false,
		downloading: false,
		selected: false,
		percent: 0
	}],
	keys: {
		hasApiKey: false,
		engine: "whisper.cpp",
		language: "ru",
		model: "whisper-small"
	},
	usage: {
		todayMinutes: 214,
		weekMinutes: 1486,
		top: [
			{
				name: "Visual Studio Code",
				minutes: 96
			},
			{
				name: "Figma",
				minutes: 41
			},
			{
				name: "Ghostty",
				minutes: 38
			}
		]
	},
	storage: {
		dataDir: "C:\\Users\\you\\AppData\\Roaming\\Mundus",
		size: "1.8 ГБ",
		parts: []
	},
	backups: [{
		id: "ark.db.backup-2026-11-07-201738",
		size: "3.9 МБ",
		modified: 1794082659398
	}, {
		id: "ark.db.backup-2026-11-04-151322",
		size: "475 КБ",
		modified: 1793805202952
	}],
	connectors: [{
		id: "greatfrontend",
		name: "GreatFrontend",
		connected: true,
		kind: "learning",
		detail: "63 задачи"
	}, {
		id: "leetcode",
		name: "LeetCode",
		connected: false,
		kind: "learning",
		detail: "Не подключено"
	}],
	hotkey: {
		running: true,
		listenAccess: true,
		capturing: false,
		held: []
	},
	plugins: {
		catalog: {
			available: [{
				id: "com.kosmos.dictation",
				name: "Диктовка",
				summary: "Речь в текст по горячей клавише",
				version: "",
				running: false,
				executable: false
			}],
			stale: false,
			url: "https://raw.githubusercontent.com/makekosmos/cortex/main/plugins/catalog.json"
		},
		installed: [{
			id: "com.kosmos.dictation",
			name: "Диктовка",
			summary: "Речь в текст по горячей клавише",
			version: "0.5.2",
			running: false,
			executable: true
		}],
		current: null
	},
	updates: {
		state: "available",
		currentVersion: "0.10.26",
		newVersion: "0.10.27",
		channel: "production",
		canInstall: false,
		percent: 0
	},
	settings: {
		autostart: true,
		telemetry: false
	}
};
function statusLabel(status) {
	switch (status) {
		case "running": return "Работает";
		case "starting": return "Запускается";
		case "stopping": return "Останавливается";
		default: return "Нет связи";
	}
}
function minutes(value) {
	const hours = Math.floor(value / 60);
	const rest = value % 60;
	if (!hours) return `${rest} мин`;
	return rest ? `${hours} ч ${rest} мин` : `${hours} ч`;
}
/**
* The product version, as the Engine reports it. The Manager's crate version is
* its own business and never stands in for it: with the Engine down the pages
* show no version rather than a wrong one.
*/
function productVersion(state) {
	return state.engine.status === "running" ? state.engine.version : "";
}

//#endregion
//#region modal.tsx
/**
* The one modal shell of the Manager: a dimmed page, a card with a hairline
* rule, a title row and a footer row. The backdrop is the card's sibling, not
* its ancestor — a press inside the card must not close it.
*/
function Modal(props) {
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "class", "absolute inset-0 items-center justify-center px-6");
		const __nsh_hv1 = element("view");
		bindProp(__nsh_hv1, "onPress", () => props.onDismiss);
		prop(__nsh_hv1, "accessibilityLabel", "Закрыть окно");
		prop(__nsh_hv1, "role", "button");
		prop(__nsh_hv1, "class", "absolute inset-0");
		prop(__nsh_hv1, "style", { backgroundColor: "#00000059" });
		const __nsh_hv2 = element("view");
		prop(__nsh_hv2, "class", "z-10 flex-col rounded-[16px] border");
		bindProp(__nsh_hv2, "style", () => ({
			width: props.width ?? 420,
			borderColor: props.colors.border,
			backgroundColor: props.colors.card
		}));
		prop(__nsh_hv2, "role", "none");
		const __nsh_hv3 = element("view");
		prop(__nsh_hv3, "class", "w-full flex-row items-center px-5 pb-3 pt-5");
		prop(__nsh_hv3, "style", { gap: 6 });
		const __nsh_hv4 = element("text");
		prop(__nsh_hv4, "class", "flex-1 text-[15px] font-semibold");
		bindProp(__nsh_hv4, "style", () => ({ color: props.colors.foreground }));
		insert(__nsh_hv4, () => props.title);
		const __nsh_hv5 = element("view");
		bindProp(__nsh_hv5, "onPress", () => props.onDismiss);
		prop(__nsh_hv5, "role", "button");
		prop(__nsh_hv5, "accessibilityLabel", "Закрыть карточку");
		prop(__nsh_hv5, "class", "h-6 w-6 items-center justify-center rounded-md");
		bindProp(__nsh_hv5, "hoverStyle", () => ({ backgroundColor: props.colors.background }));
		const __nsh_hv6 = element("text");
		prop(__nsh_hv6, "class", "text-[13px]");
		bindProp(__nsh_hv6, "style", () => ({ color: props.colors.muted }));
		const __nsh_hv7 = text("✕");
		append(__nsh_hv6, __nsh_hv7);
		append(__nsh_hv5, __nsh_hv6);
		append(__nsh_hv3, __nsh_hv4);
		append(__nsh_hv3, __nsh_hv5);
		const __nsh_hv8 = element("view");
		prop(__nsh_hv8, "class", "w-full px-5 pb-5");
		prop(__nsh_hv8, "style", { gap: 10 });
		insert(__nsh_hv8, () => props.children);
		append(__nsh_hv2, __nsh_hv3);
		append(__nsh_hv2, __nsh_hv8);
		insert(__nsh_hv2, () => props.footer ? (() => {
			const __nsh_hv9 = element("view");
			prop(__nsh_hv9, "class", "flex-row items-center justify-end border-t px-5 py-3");
			bindProp(__nsh_hv9, "style", () => ({
				borderColor: props.colors.border,
				gap: 12
			}));
			insert(__nsh_hv9, () => props.footer);
			return __nsh_hv9;
		})() : null);
		append(__nsh_hv0, __nsh_hv1);
		append(__nsh_hv0, __nsh_hv2);
		return __nsh_hv0;
	})();
}
/** A sentence in a modal: the question the user is answering. */
function note(colors, text) {
	return (() => {
		const __nsh_hv10 = element("text");
		prop(__nsh_hv10, "class", "text-[12px] leading-[18px]");
		bindProp(__nsh_hv10, "style", () => ({ color: colors.muted }));
		insert(__nsh_hv10, text);
		return __nsh_hv10;
	})();
}
function Cancel(props) {
	return component(PillButton, {
		get colors() {
			return props.colors;
		},
		get label() {
			return props.label ?? "Отмена";
		},
		get onPress() {
			return props.onPress;
		}
	});
}
function Confirm(props) {
	return component(PillButton, {
		get colors() {
			return props.colors;
		},
		get tone() {
			return props.tone ?? "accent";
		},
		get label() {
			return props.label;
		},
		get onPress() {
			return props.onPress;
		}
	});
}

//#endregion
//#region page.tsx
/**
* The same chevron as the kit's, mirrored. The kit draws one glyph pointing
* right and this renderer has no `rotate`, so the back arrow is its own path:
* the shape is symmetric about the middle of the box.
*/
const CHEVRON_LEFT = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M14.5%206l-6%206%206%206%22%2F%3E%3C%2Fsvg%3E";
/** The kit's own chevron, pointing right: the forward arrow. */
const CHEVRON_RIGHT = "data:image/svg+xml,%3Csvg%20xmlns%3D%22http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%22%20viewBox%3D%220%200%2024%2024%22%20fill%3D%22none%22%20stroke%3D%22currentColor%22%20stroke-width%3D%221.5%22%20stroke-linecap%3D%22round%22%20stroke-linejoin%3D%22round%22%3E%3Cpath%20d%3D%22M9.5%206l6%206-6%206%22%2F%3E%3C%2Fsvg%3E";
/** One half of the anchors' pill: a chevron, dimmed when there is no step. */
function Anchor(props) {
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "onPress", () => props.enabled ? props.onPress() : void 0);
		prop(__nsh_hv0, "role", "button");
		bindProp(__nsh_hv0, "accessibilityLabel", () => props.direction === "back" ? "Назад" : "Вперёд");
		prop(__nsh_hv0, "class", "items-center justify-center");
		bindProp(__nsh_hv0, "style", () => ({
			width: 34,
			height: 30,
			opacity: props.enabled ? 1 : .32
		}));
		const __nsh_hv1 = element("image");
		bindProp(__nsh_hv1, "src", () => props.direction === "back" ? CHEVRON_LEFT : CHEVRON_RIGHT);
		bindProp(__nsh_hv1, "style", () => ({
			width: 15,
			height: 15,
			tintColor: props.colors.foreground
		}));
		append(__nsh_hv0, __nsh_hv1);
		return __nsh_hv0;
	})();
}
/** Back and forward, in one pill: the two halves share a fill and a radius. */
function Anchors(props) {
	return (() => {
		const __nsh_hv2 = element("view");
		prop(__nsh_hv2, "class", "flex-row items-center");
		bindProp(__nsh_hv2, "style", () => ({
			height: 30,
			borderRadius: 8,
			overflow: "hidden",
			backgroundColor: props.colors.raised,
			marginRight: 12
		}));
		const __nsh_hv3 = element("view");
		bindProp(__nsh_hv3, "style", () => ({
			width: 1,
			height: 18,
			backgroundColor: props.colors.border
		}));
		append(__nsh_hv2, __nsh_hv3);
		insert(__nsh_hv2, component(Anchor, {
			direction: "back",
			get enabled() {
				return canBack();
			},
			get colors() {
				return props.colors;
			},
			onPress: back
		}), __nsh_hv3);
		insert(__nsh_hv2, component(Anchor, {
			direction: "forward",
			get enabled() {
				return canForward();
			},
			get colors() {
				return props.colors;
			},
			onPress: forward
		}));
		return __nsh_hv2;
	})();
}
/**
* The frame every page sits in: the scroll area, its gutter, the heading row the
* window is dragged by, and the column the sections stack in.
*
* The kit's `SettingsPage` does the same but caps the column at 1180 points,
* which leaves a strip of bare background on a wide window. A Manager window is
* the size the person gave it, so the cards take all of it.
*/
function Page(props) {
	const controls = titleBar();
	return (() => {
		const __nsh_hv4 = element("scroll");
		prop(__nsh_hv4, "class", "min-h-0 flex-1");
		bindProp(__nsh_hv4, "style", () => ({ backgroundColor: props.colors.background }));
		const __nsh_hv5 = element("view");
		prop(__nsh_hv5, "class", "w-full");
		bindProp(__nsh_hv5, "style", () => ({
			paddingLeft: settingsMetrics.pageGutter,
			paddingRight: settingsMetrics.pageGutter,
			paddingTop: 0,
			paddingBottom: settingsMetrics.pageBottom
		}));
		const __nsh_hv6 = element("view");
		prop(__nsh_hv6, "class", "flex-row items-center");
		bindProp(__nsh_hv6, "style", () => ({
			height: headerHeight(),
			paddingLeft: controls.left,
			marginRight: captionStrip()
		}));
		spread(__nsh_hv6, dragBinding());
		const __nsh_hv8 = element("view");
		prop(__nsh_hv8, "class", "flex-1");
		append(__nsh_hv6, __nsh_hv8);
		insert(__nsh_hv6, [
			component(Anchors, { get colors() {
				return props.colors;
			} }),
			() => props.title ? (() => {
				const __nsh_hv7 = element("text");
				bindProp(__nsh_hv7, "style", () => ({
					fontSize: 13,
					lineHeight: 19,
					fontWeight: "500",
					color: props.colors.foreground
				}));
				insert(__nsh_hv7, () => props.title);
				return __nsh_hv7;
			})() : null,
			() => props.actions ?? null
		], __nsh_hv8);
		const __nsh_hv9 = element("view");
		bindProp(__nsh_hv9, "style", () => ({
			gap: settingsMetrics.sectionGap,
			marginTop: settingsMetrics.headerGap
		}));
		insert(__nsh_hv9, () => props.children);
		append(__nsh_hv5, __nsh_hv6);
		append(__nsh_hv5, __nsh_hv9);
		append(__nsh_hv4, __nsh_hv5);
		return __nsh_hv4;
	})();
}

//#endregion
//#region pages/updates.tsx
/** The update card's one button: check, download, or install. Nothing is
* downloaded without this press.
*
* A plain function, not a component: the host compares a child component's props
* by identity, and the Engine patches one state object in place — a component
* boundary here would keep the first button on screen after the state moved. */
function updateAction(props) {
	const { updates } = props;
	if (updates.percent > 0 && updates.percent < 100) return (() => {
		const __nsh_hv0 = element("text");
		bindProp(__nsh_hv0, "style", () => ({
			fontSize: 12.5,
			color: props.colors.muted
		}));
		insert(__nsh_hv0, () => `${Math.round(updates.percent)}%`);
		return __nsh_hv0;
	})();
	if (updates.canInstall) return component(PillButton, {
		get colors() {
			return props.colors;
		},
		tone: "accent",
		label: "Установить",
		onPress: () => props.onCommand("updates.install")
	});
	if (updates.newVersion) return component(PillButton, {
		get colors() {
			return props.colors;
		},
		tone: "accent",
		label: "Скачать",
		onPress: () => props.onCommand("updates.download")
	});
	return component(PillButton, {
		get colors() {
			return props.colors;
		},
		label: "Проверить",
		onPress: () => props.onCommand("updates.check")
	});
}
function updateSubtitle(updates) {
	if (updates.percent > 0 && updates.percent < 100) return `Загружается · ${Math.round(updates.percent)}%`;
	if (updates.canInstall) return updates.newVersion ? `Версия ${updates.newVersion} готова к установке` : "Обновление готово к установке";
	if (updates.newVersion) return `Доступна версия ${updates.newVersion}`;
	if (updates.state === "checking") return "Проверяем…";
	if (updates.state === "error") return "Не удалось проверить обновления";
	return "Установлена последняя версия";
}

//#endregion
//#region pages/server.tsx
/** The page of the reference: what the Engine is doing right now. */
function ServerPage(props) {
	const state = () => props.state();
	const running = state().engine.status === "running";
	const busy = state().engine.status === "starting" || state().engine.status === "stopping";
	const code = state().sync.ownCode;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Сервер",
		get children() {
			return [
				component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return component(CardRow, {
							get colors() {
								return props.colors;
							},
							logo: true,
							get title() {
								return productVersion(state()) ? `Mundus ${productVersion(state())}` : "Mundus";
							},
							get subtitle() {
								return productVersion(state()) ? `${statusLabel(state().engine.status)} · канал ${state().engine.channel}` : statusLabel(state().engine.status);
							},
							last: true,
							get right() {
								return component(PillButton, {
									get colors() {
										return props.colors;
									},
									tone: running ? "danger" : "accent",
									label: running ? "Остановить" : "Запустить",
									disabled: busy,
									onPress: () => props.onCommand(running ? "server.stop" : "server.start")
								});
							}
						});
					}
				}),
				(() => {
					const __nsh_hv0 = element("view");
					insert(__nsh_hv0, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Программа";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return [component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Обновления",
								get subtitle() {
									return updateSubtitle(state().updates);
								},
								get right() {
									return updateAction({
										...props,
										updates: state().updates
									});
								}
							}), component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Engine",
								get subtitle() {
									return state().engine.version ? `${statusLabel(state().engine.status)} · ${state().engine.version}` : statusLabel(state().engine.status);
								},
								get right() {
									return component(StatusDot, {
										get colors() {
											return props.colors;
										},
										get tone() {
											return state().engine.status === "running" ? "success" : "muted";
										}
									});
								},
								last: true
							})];
						}
					})]);
					return __nsh_hv0;
				})(),
				(() => {
					const __nsh_hv1 = element("view");
					insert(__nsh_hv1, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Коннектор";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return [
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									icon: "globe",
									title: "Подключить другие устройства",
									get subtitle() {
										return state().sync.running ? "Синхронизация включена" : "Синхронизация выключена";
									},
									get right() {
										return component(PillButton, {
											get colors() {
												return props.colors;
											},
											tone: "accent",
											label: "Подключить",
											onPress: () => props.onCommand("connect.open")
										});
									}
								}),
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									icon: "key",
									title: "Код этого компьютера",
									subtitle: code ? "Введите его на другом устройстве" : "Покажите код тому, кого подключаете",
									get right() {
										return component(PillButton, {
											get colors() {
												return props.colors;
											},
											label: code ? "Скрыть" : "Показать",
											onPress: () => props.onCommand(code ? "pairing.hide" : "pairing.code")
										});
									}
								}),
								() => code ? component(FieldRow, {
									get colors() {
										return props.colors;
									},
									label: "Код",
									get children() {
										return (() => {
											const __nsh_hv2 = element("text");
											bindProp(__nsh_hv2, "style", () => ({
												fontSize: 11.5,
												color: props.colors.muted
											}));
											insert(__nsh_hv2, code);
											return __nsh_hv2;
										})();
									}
								}) : null,
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									icon: "monitor",
									title: "Девайсов подключено",
									get subtitle() {
										return state().sync.peers.length ? `${state().sync.peers.length} в списке` : "Пока ни одного";
									},
									last: true,
									get right() {
										return component(PillButton, {
											get colors() {
												return props.colors;
											},
											label: "Открыть",
											onPress: () => props.onCommand("route", { route: "devices" })
										});
									}
								})
							];
						}
					})]);
					return __nsh_hv1;
				})(),
				(() => {
					const __nsh_hv3 = element("view");
					insert(__nsh_hv3, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Компьютер";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return [component(FieldRow, {
								get colors() {
									return props.colors;
								},
								label: "Имя",
								get children() {
									return (() => {
										const __nsh_hv4 = element("text");
										bindProp(__nsh_hv4, "style", () => ({
											fontSize: 13,
											color: props.colors.foreground
										}));
										insert(__nsh_hv4, () => state().client.name);
										return __nsh_hv4;
									})();
								}
							}), component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Порт Engine",
								subtitle: "Из engine.lock.json того Engine, что отвечает",
								last: true,
								get right() {
									return (() => {
										const __nsh_hv5 = element("text");
										bindProp(__nsh_hv5, "style", () => ({
											fontSize: 12.5,
											color: props.colors.muted
										}));
										insert(__nsh_hv5, () => state().engine.port || "—");
										return __nsh_hv5;
									})();
								}
							})];
						}
					})]);
					return __nsh_hv3;
				})()
			];
		}
	});
}

//#endregion
//#region pages/system.tsx
function ActivityPage(props) {
	const usage = () => props.state().usage;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Активность",
		get children() {
			return [(() => {
				const __nsh_hv0 = element("view");
				insert(__nsh_hv0, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Экранное время";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return [component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Сегодня",
							get subtitle() {
								return minutes(usage().todayMinutes);
							}
						}), component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "За неделю",
							get subtitle() {
								return minutes(usage().weekMinutes);
							},
							last: true
						})];
					}
				})]);
				return __nsh_hv0;
			})(), (() => {
				const __nsh_hv1 = element("view");
				insert(__nsh_hv1, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Чаще всего";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return usage().top.length ? usage().top.map((program, index) => component(CardRow, {
							get key() {
								return program.name;
							},
							get colors() {
								return props.colors;
							},
							get title() {
								return program.name;
							},
							get subtitle() {
								return minutes(program.minutes);
							},
							get last() {
								return index === usage().top.length - 1;
							}
						})) : component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Пока нечего показать",
							get subtitle() {
								return platform() === "darwin" ? "Движок пока считает активность только на Windows" : "Учёт активности выключен или пуст";
							},
							last: true
						});
					}
				})]);
				return __nsh_hv1;
			})()];
		}
	});
}
function PreferencesPage(props) {
	const state = () => props.state();
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Настройки",
		get children() {
			return (() => {
				const __nsh_hv2 = element("view");
				insert(__nsh_hv2, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Общие";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return [component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Запуск при входе",
							subtitle: "Engine и Manager стартуют вместе с системой",
							get right() {
								return component(Switch, {
									get colors() {
										return props.colors;
									},
									get value() {
										return state().settings.autostart;
									},
									accessibilityLabel: "Запуск при входе",
									onChange: (value) => props.onCommand("settings.autostart", { value })
								});
							}
						}), component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Телеметрия",
							subtitle: "Общая статистика использования Engine",
							last: true,
							get right() {
								return component(Switch, {
									get colors() {
										return props.colors;
									},
									get value() {
										return state().settings.telemetry;
									},
									accessibilityLabel: "Телеметрия",
									onChange: (value) => props.onCommand("settings.telemetry", { value })
								});
							}
						})];
					}
				})]);
				return __nsh_hv2;
			})();
		}
	});
}

//#endregion
//#region pages/storage.tsx
/**
* The storage overview of the old Manager: one bar, one row per part, and a
* press that opens a part to show what is inside it. The bar is the page's
* index — the same colour marks a segment and its row.
*/
/** One colour per part, as the old Manager had them. Stable by id, so a part
* keeps its colour when the sizes move. */
function partColor(id) {
	switch (id) {
		case "database":
		case "objects": return "#f09c0e";
		case "backups":
		case "sqlite_reserved":
		case "free_pages": return "#d9b852";
		case "dictation_models":
		case "types": return "#9b87db";
		case "packages":
		case "sync": return "#659bc8";
		case "updates": return "#80ab73";
		case "file_index":
		case "search_index": return "#65b9b0";
		case "app_index":
		case "migrations": return "#b79a7e";
		case "installations":
		case "links": return "#ce8293";
		case "cache":
		case "local_state": return "#88acbe";
		case "logs": return "#a7a7a7";
		case "settings":
		case "integrations": return "#ab9cb8";
		case "runtime_state":
		case "schema": return "#8f9b7a";
		case "recordings":
		case "usage_tracker": return "#d79067";
		default: return id.startsWith("database_file") ? "#88acbe" : "#89919b";
	}
}
/** When a backup was taken, as a list row reads it. */
function stamp(milliseconds) {
	if (!milliseconds) return "";
	const date = new Date(milliseconds);
	const day = String(date.getDate()).padStart(2, "0");
	const month = String(date.getMonth() + 1).padStart(2, "0");
	const hours = String(date.getHours()).padStart(2, "0");
	const rest = String(date.getMinutes()).padStart(2, "0");
	return `${day}.${month}.${date.getFullYear()} · ${hours}:${rest}`;
}
/** Bytes as a row reads them: a decimal unit and one significance step. */
function sizeText(bytes) {
	const units = [
		"Б",
		"КБ",
		"МБ",
		"ГБ",
		"ТБ"
	];
	let value = bytes;
	let unit = 0;
	while (value >= 1e3 && unit < units.length - 1) {
		value /= 1e3;
		unit += 1;
	}
	if (unit === 0) return `${bytes} Б`;
	return value >= 10 ? `${Math.round(value)} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}
/** Biggest first, so the bar leads with what fills the disk. */
function sorted(parts) {
	return [...parts].sort((left, right) => right.bytes - left.bytes);
}
/** The bar's segments. In the database scope the bar shows the database's own
* files instead of the database as one part. */
function segments(parts, databaseOnly) {
	if (!databaseOnly) return sorted(parts);
	const database = parts.find((part) => part.id === "database");
	if (!database) return [];
	const items = (database.detail ?? []).map((item, index) => ({
		...item,
		id: item.id || `database_file_${index}`
	}));
	if (items.length === 0) return [database];
	const attributed = items.reduce((sum, item) => sum + item.bytes, 0);
	const remainder = database.bytes - attributed;
	if (remainder > 0) items.push({
		id: "sqlite_reserved",
		label: "Журнал и резерв SQLite",
		description: "Журнал изменений, память координации и свободные страницы SQLite.",
		bytes: remainder,
		size: sizeText(remainder)
	});
	return sorted(items);
}
const databaseOnly = signal(false);
const openIds = signal([]);
function toggle(id) {
	const open = openIds();
	openIds.set(open.includes(id) ? open.filter((entry) => entry !== id) : [...open, id]);
}
function StoragePage(props) {
	const state = () => props.state();
	const parts = () => segments(state().storage.parts, databaseOnly());
	const total = () => parts().reduce((sum, part) => sum + part.bytes, 0);
	const open = (id) => openIds().includes(id);
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Хранилище",
		get children() {
			return [
				(() => {
					const __nsh_hv0 = element("view");
					prop(__nsh_hv0, "class", "flex-row");
					prop(__nsh_hv0, "style", { gap: 8 });
					insert(__nsh_hv0, [component(PillButton, {
						get colors() {
							return props.colors;
						},
						get tone() {
							return databaseOnly() ? void 0 : "accent";
						},
						label: "Все",
						onPress: () => databaseOnly.set(false)
					}), component(PillButton, {
						get colors() {
							return props.colors;
						},
						get tone() {
							return databaseOnly() ? "accent" : void 0;
						},
						label: "База данных",
						onPress: () => databaseOnly.set(true)
					})]);
					return __nsh_hv0;
				})(),
				component(Card, {
					get colors() {
						return props.colors;
					},
					padded: true,
					get children() {
						return [
							(() => {
								const __nsh_hv1 = element("view");
								prop(__nsh_hv1, "style", {
									gap: 4,
									marginBottom: 12
								});
								const __nsh_hv2 = element("text");
								bindProp(__nsh_hv2, "style", () => ({
									fontSize: 28,
									lineHeight: 34,
									fontWeight: 600,
									color: props.colors.foreground
								}));
								insert(__nsh_hv2, () => sizeText(total()));
								const __nsh_hv3 = element("text");
								bindProp(__nsh_hv3, "style", () => ({
									fontSize: 12,
									lineHeight: 16,
									color: props.colors.muted
								}));
								insert(__nsh_hv3, () => databaseOnly() ? "Содержимое рабочей базы ARK и журнал SQLite" : "Установки, данные и локальные ИИ-модели Mundus");
								append(__nsh_hv1, __nsh_hv2);
								append(__nsh_hv1, __nsh_hv3);
								return __nsh_hv1;
							})(),
							(() => {
								const __nsh_hv4 = element("view");
								prop(__nsh_hv4, "class", "flex-row");
								prop(__nsh_hv4, "style", {
									height: 27,
									gap: 2,
									marginBottom: 14
								});
								insert(__nsh_hv4, () => parts().map((part) => (() => {
									const __nsh_hv5 = element("view");
									bindProp(__nsh_hv5, "key", () => part.id);
									bindProp(__nsh_hv5, "style", () => ({
										flexGrow: total() > 0 ? part.bytes / total() * 100 : 1,
										minWidth: 4,
										height: 27,
										borderRadius: 5,
										backgroundColor: partColor(part.id)
									}));
									prop(__nsh_hv5, "hoverStyle", { opacity: .72 });
									bindProp(__nsh_hv5, "tooltip", () => (() => {
										const __nsh_hv6 = element("text");
										prop(__nsh_hv6, "style", {
											fontSize: 12,
											color: "#ffffff"
										});
										insert(__nsh_hv6, () => `${part.label}: ${sizeText(part.bytes)}`);
										return __nsh_hv6;
									})());
									prop(__nsh_hv5, "onPress", () => {
										const id = part.id;
										if (!open(id) && part.detail?.length) toggle(id);
									});
									return __nsh_hv5;
								})()));
								return __nsh_hv4;
							})(),
							(() => {
								const __nsh_hv7 = element("view");
								insert(__nsh_hv7, () => parts().map((part) => (() => {
									const __nsh_hv8 = element("view");
									bindProp(__nsh_hv8, "key", () => part.id);
									const __nsh_hv9 = element("view");
									prop(__nsh_hv9, "class", "flex-row items-center");
									prop(__nsh_hv9, "style", {
										gap: 10,
										paddingVertical: 9,
										borderRadius: 8,
										paddingHorizontal: 6
									});
									prop(__nsh_hv9, "hoverStyle", { backgroundColor: "rgba(255,255,255,0.05)" });
									prop(__nsh_hv9, "onPress", () => {
										if (part.detail?.length) toggle(part.id);
									});
									const __nsh_hv10 = element("view");
									bindProp(__nsh_hv10, "style", () => ({
										width: 16,
										height: 16,
										borderRadius: 8,
										backgroundColor: partColor(part.id)
									}));
									const __nsh_hv11 = element("view");
									prop(__nsh_hv11, "style", {
										flexGrow: 1,
										flexShrink: 1,
										minWidth: 0
									});
									const __nsh_hv12 = element("text");
									bindProp(__nsh_hv12, "style", () => ({
										fontSize: 13,
										lineHeight: 18,
										color: props.colors.foreground
									}));
									insert(__nsh_hv12, () => part.label);
									append(__nsh_hv11, __nsh_hv12);
									const __nsh_hv13 = element("text");
									prop(__nsh_hv13, "class", "font-mono");
									bindProp(__nsh_hv13, "style", () => ({
										fontSize: 12.5,
										color: props.colors.muted
									}));
									insert(__nsh_hv13, () => sizeText(part.bytes));
									append(__nsh_hv9, __nsh_hv10);
									append(__nsh_hv9, __nsh_hv11);
									append(__nsh_hv9, __nsh_hv13);
									append(__nsh_hv8, __nsh_hv9);
									insert(__nsh_hv8, () => open(part.id) && part.detail?.length ? (() => {
										const __nsh_hv14 = element("view");
										prop(__nsh_hv14, "style", {
											paddingLeft: 32,
											paddingBottom: 8
										});
										insert(__nsh_hv14, () => part.detail.map((item, index) => (() => {
											const __nsh_hv15 = element("view");
											bindProp(__nsh_hv15, "key", () => `${part.id}-${item.id}-${index}`);
											prop(__nsh_hv15, "class", "flex-row items-center");
											prop(__nsh_hv15, "style", {
												gap: 10,
												paddingVertical: 4
											});
											const __nsh_hv16 = element("view");
											prop(__nsh_hv16, "style", {
												flexGrow: 1,
												flexShrink: 1,
												minWidth: 0
											});
											const __nsh_hv17 = element("text");
											bindProp(__nsh_hv17, "style", () => ({
												fontSize: 12,
												lineHeight: 16,
												color: props.colors.muted
											}));
											insert(__nsh_hv17, () => item.label || item.id);
											append(__nsh_hv16, __nsh_hv17);
											const __nsh_hv18 = element("text");
											prop(__nsh_hv18, "class", "font-mono");
											bindProp(__nsh_hv18, "style", () => ({
												fontSize: 12,
												color: props.colors.muted
											}));
											insert(__nsh_hv18, () => sizeText(item.bytes));
											append(__nsh_hv15, __nsh_hv16);
											append(__nsh_hv15, __nsh_hv18);
											return __nsh_hv15;
										})()));
										return __nsh_hv14;
									})() : null);
									return __nsh_hv8;
								})()));
								return __nsh_hv7;
							})()
						];
					}
				}),
				(() => {
					const __nsh_hv19 = element("view");
					insert(__nsh_hv19, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Данные";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return (() => {
								const __nsh_hv20 = element("view");
								prop(__nsh_hv20, "style", {
									paddingVertical: 10,
									gap: 2
								});
								const __nsh_hv21 = element("text");
								bindProp(__nsh_hv21, "style", () => ({
									fontSize: 13,
									lineHeight: 18,
									color: props.colors.foreground
								}));
								const __nsh_hv22 = text("Папка данных");
								append(__nsh_hv21, __nsh_hv22);
								const __nsh_hv23 = element("text");
								bindProp(__nsh_hv23, "style", () => ({
									fontSize: 11.5,
									lineHeight: 16,
									color: props.colors.muted
								}));
								insert(__nsh_hv23, () => state().storage.dataDir);
								append(__nsh_hv20, __nsh_hv21);
								append(__nsh_hv20, __nsh_hv23);
								return __nsh_hv20;
							})();
						}
					})]);
					return __nsh_hv19;
				})(),
				(() => {
					const __nsh_hv24 = element("view");
					insert(__nsh_hv24, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Резервные копии";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return [() => state().backups.length ? state().backups.map((backup, index, list) => component(CardRow, {
								get key() {
									return backup.id;
								},
								get colors() {
									return props.colors;
								},
								get title() {
									return backup.id;
								},
								get subtitle() {
									return [stamp(backup.modified), backup.size].filter(Boolean).join(" · ");
								},
								get last() {
									return index === list.length - 1;
								},
								get right() {
									return component(PillButton, {
										get colors() {
											return props.colors;
										},
										tone: "danger",
										label: "Восстановить",
										onPress: () => props.onCommand("backups.restore", { id: backup.id })
									});
								}
							})) : component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Копий нет",
								subtitle: "Engine ещё не сделал ни одной",
								last: true
							}), component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Сделать копию сейчас",
								subtitle: "Снимок базы рядом с данными Engine",
								last: true,
								get right() {
									return component(PillButton, {
										get colors() {
											return props.colors;
										},
										tone: "accent",
										label: "Создать",
										onPress: () => props.onCommand("backups.create")
									});
								}
							})];
						}
					})]);
					return __nsh_hv24;
				})()
			];
		}
	});
}

//#endregion
//#region pages/library.tsx
function ModelsPage(props) {
	const models = () => props.state().models;
	const stateOf = (model) => {
		if (model.downloading) return `Загружается · ${Math.round(model.percent)}%`;
		if (model.downloaded) return model.selected ? `${model.size} · выбрана` : `${model.size} · загружена`;
		return `${model.size} · не загружена`;
	};
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Модели",
		get children() {
			return [(() => {
				const __nsh_hv0 = element("view");
				insert(__nsh_hv0, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Локальные модели";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return models().length ? models().map((model, index) => component(CardRow, {
							get key() {
								return model.id;
							},
							get colors() {
								return props.colors;
							},
							icon: "chip",
							get iconFill() {
								return props.colors.raised;
							},
							get accent() {
								return props.colors.muted;
							},
							get title() {
								return model.name;
							},
							get subtitle() {
								return stateOf(model);
							},
							get last() {
								return index === models().length - 1;
							},
							get right() {
								return (() => {
									const __nsh_hv1 = element("view");
									prop(__nsh_hv1, "class", "flex-row items-center gap-2");
									insert(__nsh_hv1, () => model.downloading ? (() => {
										const __nsh_hv2 = element("text");
										bindProp(__nsh_hv2, "style", () => ({
											fontSize: 12.5,
											color: props.colors.muted
										}));
										insert(__nsh_hv2, () => `${Math.round(model.percent)}%`);
										return __nsh_hv2;
									})() : model.downloaded ? [
										() => model.selected ? component(StatusDot, { get colors() {
											return props.colors;
										} }) : null,
										() => !model.selected ? component(PillButton, {
											get colors() {
												return props.colors;
											},
											tone: "accent",
											label: "Выбрать",
											onPress: () => props.onCommand("models.select", { id: model.id })
										}) : null,
										component(PillButton, {
											get colors() {
												return props.colors;
											},
											tone: "danger",
											label: "Удалить",
											onPress: () => props.onCommand("models.delete", { id: model.id })
										})
									] : component(PillButton, {
										get colors() {
											return props.colors;
										},
										tone: "accent",
										label: "Скачать",
										onPress: () => props.onCommand("models.download", { id: model.id })
									}));
									return __nsh_hv1;
								})();
							}
						})) : component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Список пуст",
							subtitle: "Engine не предложил ни одной модели",
							last: true
						});
					}
				})]);
				return __nsh_hv0;
			})(), (() => {
				const __nsh_hv3 = element("text");
				bindProp(__nsh_hv3, "style", () => ({
					fontSize: 12,
					color: props.colors.faint
				}));
				const __nsh_hv4 = text("Модели распознавания живут на этом компьютере: звук не уходит в сеть.");
				append(__nsh_hv3, __nsh_hv4);
				return __nsh_hv3;
			})()];
		}
	});
}
const draft = signal("");
const verifyNote = signal("");
function KeysPage(props) {
	const keys = () => props.state().keys;
	const save = () => {
		const value = draft();
		if (!value.trim()) return;
		props.onCommand("keys.save", { value });
		draft.set("");
		verifyNote.set("Ключ отправлен в Engine");
	};
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Ключи",
		get children() {
			return [
				(() => {
					const __nsh_hv5 = element("view");
					insert(__nsh_hv5, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Диктовка";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return [
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									icon: "key",
									get iconFill() {
										return props.colors.raised;
									},
									get accent() {
										return keys().hasApiKey ? props.colors.success : props.colors.muted;
									},
									title: "Ключ распознавания",
									get subtitle() {
										return keys().hasApiKey ? "Сохранён в Engine" : "Не сохранён — работает только локальная модель";
									},
									get right() {
										return component(StatusDot, {
											get colors() {
												return props.colors;
											},
											get tone() {
												return keys().hasApiKey ? "success" : "muted";
											}
										});
									}
								}),
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									title: "Движок",
									get subtitle() {
										return keys().engine || "локальный";
									}
								}),
								component(CardRow, {
									get colors() {
										return props.colors;
									},
									title: "Язык",
									get subtitle() {
										return keys().language || "не выбран";
									},
									get last() {
										return !verifyNote();
									}
								}),
								() => verifyNote() ? component(CardRow, {
									get colors() {
										return props.colors;
									},
									title: "Проверка",
									get subtitle() {
										return verifyNote();
									},
									last: true
								}) : null
							];
						}
					})]);
					return __nsh_hv5;
				})(),
				(() => {
					const __nsh_hv6 = element("view");
					insert(__nsh_hv6, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return keys().hasApiKey ? "Заменить ключ" : "Добавить ключ";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						padded: true,
						get children() {
							return (() => {
								const __nsh_hv7 = element("view");
								prop(__nsh_hv7, "style", { gap: 10 });
								const __nsh_hv8 = element("view");
								prop(__nsh_hv8, "class", "flex-row items-center justify-end gap-2");
								insert(__nsh_hv8, [component(PillButton, {
									get colors() {
										return props.colors;
									},
									label: "Проверить",
									onPress: () => props.onCommand("keys.verify", { value: draft() })
								}), component(PillButton, {
									get colors() {
										return props.colors;
									},
									tone: "accent",
									label: "Сохранить",
									onPress: save
								})]);
								append(__nsh_hv7, __nsh_hv8);
								insert(__nsh_hv7, component(TextInput, {
									get colors() {
										return props.colors;
									},
									get value() {
										return draft();
									},
									placeholder: "Вставьте ключ",
									accessibilityLabel: "Ключ диктовки",
									onChange: (value) => {
										draft.set(value);
										verifyNote.set("");
									},
									onSubmit: save
								}), __nsh_hv8);
								return __nsh_hv7;
							})();
						}
					})]);
					return __nsh_hv6;
				})(),
				(() => {
					const __nsh_hv9 = element("text");
					bindProp(__nsh_hv9, "style", () => ({
						fontSize: 12,
						color: props.colors.faint
					}));
					const __nsh_hv10 = text("Ключ хранится в связке ключей Engine. Manager его не читает и нигде не показывает.");
					append(__nsh_hv9, __nsh_hv10);
					return __nsh_hv9;
				})()
			];
		}
	});
}

//#endregion
//#region pages/plugin-fields.tsx
/** A choice as a row of buttons: one is on, the rest ask to be. */
function ChoiceRow(props) {
	const { colors, value } = props;
	return component(FieldRow, {
		colors,
		get label() {
			return props.label;
		},
		get last() {
			return props.last;
		},
		labelWidth: 210,
		get children() {
			return (() => {
				const __nsh_hv0 = element("view");
				prop(__nsh_hv0, "class", "flex-row flex-wrap items-center");
				prop(__nsh_hv0, "style", { gap: 6 });
				insert(__nsh_hv0, () => props.options.map((option) => component(PillButton, {
					get key() {
						return option.label;
					},
					colors,
					get tone() {
						return same(option.value, value) ? "accent" : "neutral";
					},
					get label() {
						return option.label;
					},
					onPress: () => props.onPick(option.value)
				})));
				return __nsh_hv0;
			})();
		}
	});
}
/** Two stored values are the same choice: `"ru"` with `"ru"`, `600000` with
* `600000`, `null` with `null` — not `"5"` with `5`. */
function same(option, value) {
	return JSON.stringify(option ?? null) === JSON.stringify(value ?? null);
}
/** The open picker: which field, and what has been typed into it. One at a
* time, so the page never stacks two lists on top of each other. */
const picking = signal({
	key: "",
	query: ""
});
/** A long choice (a hundred languages) as one current value and a searchable
* list behind it — a row of a hundred buttons is not a choice, it is a wall. */
function PickerRow(props) {
	const { colors } = props;
	const selected = props.options.find((option) => same(option.value, props.value));
	return component(FieldRow, {
		colors,
		get label() {
			return props.label;
		},
		get last() {
			return props.last;
		},
		labelWidth: 210,
		get children() {
			return (() => {
				const __nsh_hv1 = element("view");
				prop(__nsh_hv1, "class", "w-full flex-row justify-end");
				insert(__nsh_hv1, component(PillButton, {
					colors,
					tone: "neutral",
					get label() {
						return selected?.label ?? "Не выбрано";
					},
					onPress: () => picking.set({
						key: props.fieldKey,
						query: ""
					})
				}));
				return __nsh_hv1;
			})();
		}
	});
}
/** The list a `PickerRow` opens: drawn beside the page, not inside the row, so
* it covers the window the way every other modal of the Manager does. */
function PickerModal(props) {
	const { colors } = props;
	const close = () => picking.set({
		key: "",
		query: ""
	});
	const matching = () => {
		const needle = picking().query.trim().toLowerCase();
		if (!needle) return props.options;
		return props.options.filter((option) => option.label.toLowerCase().includes(needle));
	};
	return component(Modal, {
		get title() {
			return props.title;
		},
		colors,
		onDismiss: close,
		width: 380,
		get children() {
			return [component(SearchField, {
				colors,
				get value() {
					return picking().query;
				},
				placeholder: "Поиск",
				onChange: (next) => picking.set({
					key: picking().key,
					query: next
				})
			}), (() => {
				const __nsh_hv2 = element("scroll");
				prop(__nsh_hv2, "class", "min-h-0");
				prop(__nsh_hv2, "style", { height: 320 });
				insert(__nsh_hv2, () => matching().map((option) => (() => {
					const __nsh_hv3 = element("view");
					bindProp(__nsh_hv3, "key", () => option.label);
					prop(__nsh_hv3, "role", "button");
					bindProp(__nsh_hv3, "accessibilityLabel", () => option.label);
					prop(__nsh_hv3, "onPress", () => {
						close();
						props.onPick(option.value);
					});
					prop(__nsh_hv3, "class", "h-9 flex-row items-center justify-between rounded-md px-2");
					bindProp(__nsh_hv3, "hoverStyle", () => ({ backgroundColor: colors.background }));
					const __nsh_hv4 = element("text");
					prop(__nsh_hv4, "class", "text-[13px]");
					bindProp(__nsh_hv4, "style", () => ({ color: colors.foreground }));
					insert(__nsh_hv4, () => option.label);
					append(__nsh_hv3, __nsh_hv4);
					insert(__nsh_hv3, () => same(option.value, props.value) ? (() => {
						const __nsh_hv5 = element("text");
						prop(__nsh_hv5, "class", "text-[13px]");
						bindProp(__nsh_hv5, "style", () => ({ color: colors.accent }));
						const __nsh_hv6 = text("✓");
						append(__nsh_hv5, __nsh_hv6);
						return __nsh_hv5;
					})() : null);
					return __nsh_hv3;
				})()));
				return __nsh_hv2;
			})()];
		}
	});
}
/** The label column: wide enough for a two-word setting name, so a field never
* wraps into the control's row (`Подсказка для расшифровки` is the longest). */
const LABEL_WIDTH = 210;

//#endregion
//#region pages/plugin-hotkey.tsx
/** `FnFn` is a double tap, shown as one block: `Fn + Fn`. */
function hotkeyLabel(value) {
	const text = value.trim();
	if (!text) return "Не задано";
	if (text.includes("+")) return text.split("+").map((part) => part.trim()).filter(Boolean).join(" + ");
	const half = Math.floor(text.length / 2);
	if (text.length >= 2 && text.length % 2 === 0 && text.slice(0, half) === text.slice(half)) return `${text.slice(0, half)} + ${text.slice(half)}`;
	return text;
}
/** The glyph of each modifier the Engine names in a hotkey string. */
const KEYCAPS = /* @__PURE__ */ new Map([
	["ctrl", "⌃"],
	["alt", "⌥"],
	["shift", "⇧"],
	["cmd", "⌘"],
	["fn", "Fn"]
]);
/** A modifier's glyph, or the name itself when it has none. */
function keycap(name) {
	return KEYCAPS.get(name) ?? name;
}
function Keycap(props) {
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "class", "items-center justify-center");
		bindProp(__nsh_hv0, "style", () => ({
			minWidth: 26,
			height: 26,
			paddingLeft: 6,
			paddingRight: 6,
			borderRadius: 7,
			backgroundColor: props.dim ? props.colors.card : props.colors.background,
			borderWidth: 1,
			borderColor: props.colors.border
		}));
		const __nsh_hv1 = element("text");
		bindProp(__nsh_hv1, "style", () => ({
			fontSize: 13,
			color: props.dim ? props.colors.faint : props.colors.foreground
		}));
		insert(__nsh_hv1, () => props.label);
		append(__nsh_hv0, __nsh_hv1);
		return __nsh_hv0;
	})();
}
/**
* The hotkey picker, the way Raycast shows one: a chip that becomes a small
* keycap panel while the person presses a combination. Nothing is typed — the
* Manager's own event tap reads the keys, so the result is the same whether
* this window has the focus or not.
*/
function HotkeyRecorder(props) {
	if (props.capturing) return (() => {
		const __nsh_hv2 = element("view");
		prop(__nsh_hv2, "class", "w-full flex-col items-end");
		prop(__nsh_hv2, "style", { rowGap: 6 });
		const __nsh_hv3 = element("view");
		prop(__nsh_hv3, "class", "flex-row items-center");
		bindProp(__nsh_hv3, "style", () => ({
			columnGap: 6,
			padding: 6,
			borderRadius: 10,
			backgroundColor: props.colors.raised,
			borderWidth: 1,
			borderColor: props.colors.accent
		}));
		insert(__nsh_hv3, () => props.held.length ? props.held.map((key) => component(Keycap, {
			key,
			get label() {
				return keycap(key);
			},
			get colors() {
				return props.colors;
			}
		})) : (() => {
			const __nsh_hv4 = element("text");
			bindProp(__nsh_hv4, "style", () => ({
				fontSize: 12,
				color: props.colors.muted,
				paddingLeft: 6,
				paddingRight: 6
			}));
			const __nsh_hv5 = text("Нажми сочетание");
			append(__nsh_hv4, __nsh_hv5);
			return __nsh_hv4;
		})());
		const __nsh_hv6 = element("text");
		bindProp(__nsh_hv6, "style", () => ({
			fontSize: 11,
			color: props.colors.muted
		}));
		const __nsh_hv7 = text("Esc — отмена");
		append(__nsh_hv6, __nsh_hv7);
		append(__nsh_hv2, __nsh_hv3);
		append(__nsh_hv2, __nsh_hv6);
		return __nsh_hv2;
	})();
	return (() => {
		const __nsh_hv8 = element("view");
		prop(__nsh_hv8, "class", "w-full flex-col items-end");
		prop(__nsh_hv8, "style", { rowGap: 4 });
		const __nsh_hv9 = element("text");
		bindProp(__nsh_hv9, "onPress", () => props.onStart);
		bindProp(__nsh_hv9, "style", () => ({
			fontSize: 12,
			color: props.colors.foreground,
			backgroundColor: props.colors.raised,
			paddingLeft: 10,
			paddingRight: 10,
			paddingTop: 4,
			paddingBottom: 4,
			borderRadius: 8,
			borderWidth: 1,
			borderColor: props.colors.border
		}));
		insert(__nsh_hv9, () => hotkeyLabel(props.value));
		append(__nsh_hv8, __nsh_hv9);
		insert(__nsh_hv8, () => props.listenAccess ? null : (() => {
			const __nsh_hv10 = element("text");
			bindProp(__nsh_hv10, "onPress", () => props.onOpenSettings);
			bindProp(__nsh_hv10, "style", () => ({
				fontSize: 11,
				color: props.colors.muted
			}));
			const __nsh_hv11 = text("Хоткей молчит: нет «Мониторинга ввода» — открыть настройки");
			append(__nsh_hv10, __nsh_hv11);
			return __nsh_hv10;
		})());
		return __nsh_hv8;
	})();
}

//#endregion
//#region pages/plugin-rows.tsx
function versionLine(plugin) {
	const parts = [];
	if (plugin.version) parts.push(`Версия ${plugin.version}`);
	if (plugin.running) parts.push("запущен");
	return parts.join(" · ");
}
/** A row of the installed list: open it, change its settings, remove it. */
function InstalledRow(props) {
	const { colors, plugin } = props;
	return component(CardRow, {
		colors,
		icon: "plug",
		get accent() {
			return colors.accent;
		},
		get accentTo() {
			return colors.accent;
		},
		get title() {
			return plugin.name;
		},
		get subtitle() {
			return versionLine(plugin) || plugin.summary;
		},
		get last() {
			return props.last;
		},
		get right() {
			return (() => {
				const __nsh_hv0 = element("view");
				prop(__nsh_hv0, "class", "flex-row items-center gap-2");
				insert(__nsh_hv0, [
					component(PillButton, {
						colors,
						label: "Настроить",
						get onPress() {
							return props.onConfigure;
						}
					}),
					() => plugin.executable ? component(PillButton, {
						colors,
						tone: "neutral",
						label: "Открыть",
						get onPress() {
							return props.onOpen;
						}
					}) : null,
					component(PillButton, {
						colors,
						tone: "danger",
						label: "Удалить",
						get onPress() {
							return props.onRemove;
						}
					})
				]);
				return __nsh_hv0;
			})();
		}
	});
}
/** One catalog entry the Engine has not installed yet. */
function AvailableRow(props) {
	const { colors, plugin } = props;
	return component(CardRow, {
		colors,
		icon: "apps",
		get accent() {
			return colors.muted;
		},
		get accentTo() {
			return colors.muted;
		},
		get title() {
			return plugin.name;
		},
		get subtitle() {
			return plugin.summary;
		},
		get last() {
			return props.last;
		},
		get right() {
			return component(PillButton, {
				colors,
				tone: "accent",
				label: "Установить",
				get onPress() {
					return props.onInstall;
				}
			});
		}
	});
}

//#endregion
//#region pages/plugin-stats.tsx
/** Thousands separated by a space: `13 205`, not `13205`. */
function grouped(value) {
	const digits = String(Math.abs(Math.round(value)));
	let out = "";
	for (let at = 0; at < digits.length; at += 1) {
		if (at > 0 && (digits.length - at) % 3 === 0) out += " ";
		out += digits[at];
	}
	return value < 0 ? `-${out}` : out;
}
/** Seconds as time the way a person reads it: «2 ч 5 мин», «40 сек». */
function asDuration(seconds) {
	const total = Math.max(0, Math.round(seconds));
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor(total % 3600 / 60);
	if (hours > 0) return `${grouped(hours)} ч ${minutes} мин`;
	if (minutes > 0) return `${minutes} мин`;
	return `${total} сек`;
}
/** One number as the manifest asked for it to be printed. */
function statValue(field) {
	if (field.value === null || field.value === void 0) return "—";
	const number = Number(field.value);
	if (!Number.isFinite(number)) return String(field.value);
	if (field.format === "duration") return asDuration(number);
	return grouped(number);
}
/** The numbers a plugin reports, resolved by the Engine: a page that names no
* operation can still show any plugin's statistics. */
function StatsBlock(props) {
	const { colors, stats } = props;
	if (!stats.fields.length) return null;
	return (() => {
		const __nsh_hv0 = element("view");
		insert(__nsh_hv0, [component(SectionLabel, {
			colors,
			get children() {
				return stats.title || "Статистика";
			}
		}), component(Card, {
			colors,
			get children() {
				return stats.fields.map((field, index) => component(FieldRow, {
					get key() {
						return field.key;
					},
					colors,
					get label() {
						return field.label;
					},
					labelWidth: 210,
					get last() {
						return index === stats.fields.length - 1;
					},
					get children() {
						return (() => {
							const __nsh_hv1 = element("text");
							bindProp(__nsh_hv1, "style", () => ({
								fontSize: 15,
								color: colors.foreground
							}));
							insert(__nsh_hv1, () => statValue(field));
							return __nsh_hv1;
						})();
					}
				}));
			}
		})]);
		return __nsh_hv0;
	})();
}

//#endregion
//#region pages/plugins.tsx
/**
* What Mundus can gain and what it already has.
*
* The catalog is the Engine's (`plugins.catalog`), the install is its too
* (`plugins.install`), and this page only names what it is told — a new plugin
* appears here because it is in the catalog, not because the Manager was built
* knowing it.
*/
function PluginsPage(props) {
	const state = () => props.state().plugins;
	const installed = () => state().installed;
	const available = () => state().catalog.available;
	const isInstalled = (id) => installed().some((plugin) => plugin.id === id);
	const rest = () => available().filter((plugin) => !isInstalled(plugin.id));
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Плагины",
		get children() {
			return [(() => {
				const __nsh_hv0 = element("view");
				insert(__nsh_hv0, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Установленные";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return installed().length ? installed().map((plugin, index) => component(InstalledRow, {
							get key() {
								return plugin.id;
							},
							plugin,
							get colors() {
								return props.colors;
							},
							get last() {
								return index === installed().length - 1;
							},
							onOpen: () => props.onCommand("plugins.launch", { id: plugin.id }),
							onConfigure: () => props.onCommand("route", { route: `plugin/${plugin.id}` }),
							onRemove: () => props.onCommand("plugins.remove", { id: plugin.id })
						})) : component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "plug",
							get iconFill() {
								return props.colors.raised;
							},
							get accent() {
								return props.colors.muted;
							},
							title: "Пока ничего не установлено",
							subtitle: "Выберите плагин из списка ниже",
							last: true
						});
					}
				})]);
				return __nsh_hv0;
			})(), (() => {
				const __nsh_hv1 = element("view");
				insert(__nsh_hv1, [
					component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get right() {
							return state().catalog.stale ? (() => {
								const __nsh_hv2 = element("text");
								bindProp(__nsh_hv2, "style", () => ({
									fontSize: 12,
									color: props.colors.muted
								}));
								const __nsh_hv3 = text("Список сохранён: Engine не смог обновить его");
								append(__nsh_hv2, __nsh_hv3);
								return __nsh_hv2;
							})() : null;
						},
						get children() {
							return "Доступные";
						}
					}),
					component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return rest().length ? rest().map((plugin, index) => component(AvailableRow, {
								get key() {
									return plugin.id;
								},
								plugin,
								get colors() {
									return props.colors;
								},
								get last() {
									return index === rest().length - 1;
								},
								onInstall: () => props.onCommand("plugins.install", { id: plugin.id })
							})) : component(CardRow, {
								get colors() {
									return props.colors;
								},
								icon: "check",
								get iconFill() {
									return props.colors.raised;
								},
								get accent() {
									return props.colors.muted;
								},
								get title() {
									return available().length ? "Всё установлено" : "Список пуст";
								},
								get subtitle() {
									return available().length ? "Новые плагины появятся здесь сами" : "Engine не получил каталог плагинов";
								},
								last: true
							});
						}
					}),
					() => rest().length ? (() => {
						const __nsh_hv4 = element("view");
						prop(__nsh_hv4, "style", {
							marginTop: 8,
							paddingLeft: 12
						});
						const __nsh_hv5 = element("text");
						bindProp(__nsh_hv5, "style", () => ({
							fontSize: 12,
							lineHeight: 16,
							color: props.colors.muted
						}));
						const __nsh_hv6 = text("Загрузка идёт в фоне — плагин появится в списке выше, когда Engine его установит.");
						append(__nsh_hv5, __nsh_hv6);
						append(__nsh_hv4, __nsh_hv5);
						return __nsh_hv4;
					})() : null
				]);
				return __nsh_hv1;
			})()];
		}
	});
}
/** The values a page has typed but not sent yet, per plugin. */
const drafts = signal({
	id: "",
	values: {}
});
/**
* One plugin's page, drawn from its manifest: the Engine stores the settings,
* the plugin reads them from the file the manifest names, and this page writes
* nothing the manifest does not declare.
*/
function PluginPage(props) {
	const current = () => props.state().plugins.current;
	const manifest = () => {
		const plugin = current();
		return plugin && plugin.id === props.id ? plugin.manifest : null;
	};
	const stored = () => {
		const plugin = current();
		return plugin && plugin.id === props.id ? plugin.values : {};
	};
	const typed = () => drafts().id === props.id ? drafts().values : {};
	const dirty = () => Object.keys(typed()).length > 0;
	const valueOf = (key) => key in typed() ? typed()[key] : stored()[key];
	const draft = (key, value) => drafts.set({
		id: props.id,
		values: {
			...typed(),
			[key]: value
		}
	});
	const send = (key, value) => props.onCommand("plugins.set", {
		id: props.id,
		values: { [key]: value }
	});
	const save = () => {
		if (!dirty()) return;
		props.onCommand("plugins.set", {
			id: props.id,
			values: typed()
		});
		drafts.set({
			id: props.id,
			values: {}
		});
	};
	effect(() => {
		const captured = hotkeyCaptured();
		if (!captured) return;
		hotkeyCaptured.set("");
		send("hotkey", captured);
	});
	const plugin = () => props.state().plugins.installed.find((row) => row.id === props.id);
	const stats = () => {
		const row = current();
		return row && row.id === props.id ? row.stats ?? null : null;
	};
	const shown = manifest();
	/** The choice whose list is open, if any: `picking` holds its key. */
	const pickingField = () => {
		const key = picking().key;
		const field = shown?.settings.find((row) => row.key === key);
		if (!field || field.kind.type !== "choice") return null;
		return {
			key: field.key,
			label: field.label,
			options: field.kind.options
		};
	};
	if (!shown) return component(Page, {
		get colors() {
			return props.colors;
		},
		get title() {
			return plugin()?.name ?? "Плагин";
		},
		get children() {
			return component(Card, {
				get colors() {
					return props.colors;
				},
				get children() {
					return component(CardRow, {
						get colors() {
							return props.colors;
						},
						icon: "plug",
						get iconFill() {
							return props.colors.raised;
						},
						get accent() {
							return props.colors.muted;
						},
						title: "Настройки недоступны",
						subtitle: "Engine не сообщил описание плагина",
						last: true
					});
				}
			});
		}
	});
	return [component(Page, {
		get colors() {
			return props.colors;
		},
		get title() {
			return shown.name;
		},
		get actions() {
			return dirty() ? (() => {
				const __nsh_hv7 = element("view");
				prop(__nsh_hv7, "style", { marginLeft: 12 });
				insert(__nsh_hv7, component(PillButton, {
					get colors() {
						return props.colors;
					},
					tone: "accent",
					label: "Сохранить",
					onPress: save
				}));
				return __nsh_hv7;
			})() : null;
		},
		get children() {
			return [
				(() => {
					const __nsh_hv8 = element("view");
					insert(__nsh_hv8, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Плагин";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return component(CardRow, {
								get colors() {
									return props.colors;
								},
								icon: "plug",
								get accent() {
									return props.colors.accent;
								},
								get accentTo() {
									return props.colors.accent;
								},
								get title() {
									return shown.name;
								},
								get subtitle() {
									return shown.summary;
								},
								last: true,
								get right() {
									return (() => {
										const __nsh_hv9 = element("view");
										prop(__nsh_hv9, "class", "flex-row items-center gap-2");
										insert(__nsh_hv9, [() => plugin()?.running ? component(StatusDot, { get colors() {
											return props.colors;
										} }) : null, () => plugin()?.executable ? component(PillButton, {
											get colors() {
												return props.colors;
											},
											tone: "neutral",
											label: "Открыть",
											onPress: () => props.onCommand("plugins.launch", { id: props.id })
										}) : null]);
										return __nsh_hv9;
									})();
								}
							});
						}
					})]);
					return __nsh_hv8;
				})(),
				() => stats() ? component(StatsBlock, {
					get stats() {
						return stats();
					},
					get colors() {
						return props.colors;
					}
				}) : null,
				(() => {
					const __nsh_hv10 = element("view");
					insert(__nsh_hv10, [
						component(SectionLabel, {
							get colors() {
								return props.colors;
							},
							get children() {
								return "Настройки";
							}
						}),
						component(Card, {
							get colors() {
								return props.colors;
							},
							get children() {
								return shown.settings.map((field, index) => {
									const last = index === shown.settings.length - 1;
									const value = valueOf(field.key);
									if (field.kind.type === "toggle") return component(CardRow, {
										get key() {
											return field.key;
										},
										get colors() {
											return props.colors;
										},
										get title() {
											return field.label;
										},
										get subtitle() {
											return field.hint;
										},
										last,
										get right() {
											return component(Switch, {
												get colors() {
													return props.colors;
												},
												get value() {
													return Boolean(value);
												},
												get accessibilityLabel() {
													return field.label;
												},
												onChange: (on) => send(field.key, on)
											});
										}
									});
									const kind = field.kind;
									if (kind.type === "switch") return component(CardRow, {
										get key() {
											return field.key;
										},
										get colors() {
											return props.colors;
										},
										get title() {
											return field.label;
										},
										get subtitle() {
											return field.hint;
										},
										last,
										get right() {
											return component(Switch, {
												get colors() {
													return props.colors;
												},
												get value() {
													return !same(kind.off, value) && Boolean(value);
												},
												get accessibilityLabel() {
													return field.label;
												},
												onChange: (on) => send(field.key, on ? kind.on : kind.off)
											});
										}
									});
									if (kind.type === "choice") {
										const options = kind.options;
										if (field.key === "localModel" || options.length > 6) return component(PickerRow, {
											get key() {
												return field.key;
											},
											get fieldKey() {
												return field.key;
											},
											get label() {
												return field.label;
											},
											options,
											value,
											get colors() {
												return props.colors;
											},
											last
										});
										return component(ChoiceRow, {
											get key() {
												return field.key;
											},
											get label() {
												return field.label;
											},
											options,
											value,
											get colors() {
												return props.colors;
											},
											last,
											onPick: (next) => send(field.key, next)
										});
									}
									const numeric = field.kind.type === "number";
									if (field.key === "hotkey") {
										const hotkey = () => props.state().hotkey;
										return component(FieldRow, {
											get key() {
												return field.key;
											},
											get colors() {
												return props.colors;
											},
											get label() {
												return field.label;
											},
											last,
											labelWidth: 210,
											get children() {
												return component(HotkeyRecorder, {
													get value() {
														return String(value ?? "");
													},
													get colors() {
														return props.colors;
													},
													get capturing() {
														return hotkey().capturing;
													},
													get held() {
														return hotkey().held;
													},
													get listenAccess() {
														return hotkey().listenAccess;
													},
													onStart: () => props.onCommand("hotkey.capture"),
													onOpenSettings: () => props.onCommand("hotkey.settings")
												});
											}
										});
									}
									return component(FieldRow, {
										get key() {
											return field.key;
										},
										get colors() {
											return props.colors;
										},
										get label() {
											return field.label;
										},
										last,
										labelWidth: 210,
										get children() {
											return component(TextInput, {
												get colors() {
													return props.colors;
												},
												get value() {
													return String(value ?? "");
												},
												placeholder: numeric ? "Число" : "Не задано",
												get accessibilityLabel() {
													return field.label;
												},
												onChange: (text) => draft(field.key, numeric ? Number(text) : text),
												onSubmit: () => {
													const next = valueOf(field.key);
													if (next !== void 0) send(field.key, next);
													drafts.set({
														id: props.id,
														values: {}
													});
												}
											});
										}
									});
								});
							}
						}),
						() => shown.settings.length ? null : note(props.colors, "У этого плагина нет настроек.")
					]);
					return __nsh_hv10;
				})()
			];
		}
	}), () => (() => {
		const picked = pickingField();
		return picked ? component(PickerModal, {
			get title() {
				return picked.label;
			},
			get options() {
				return picked.options;
			},
			get value() {
				return valueOf(picked.key);
			},
			get colors() {
				return props.colors;
			},
			onPick: (next) => send(picked.key, next)
		}) : null;
	})()];
}

//#endregion
//#region pages/network.tsx
function DevicesPage(props) {
	const state = () => props.state();
	const peers = () => state().sync.peers;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Девайсы",
		get children() {
			return [
				(() => {
					const __nsh_hv0 = element("view");
					insert(__nsh_hv0, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Этот компьютер";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return component(CardRow, {
								get colors() {
									return props.colors;
								},
								icon: "monitor",
								get iconFill() {
									return props.colors.raised;
								},
								get accent() {
									return props.colors.accent;
								},
								get title() {
									return state().client.name;
								},
								get subtitle() {
									return state().sync.running ? "Текущее устройство · синхронизация включена" : "Текущее устройство";
								},
								get right() {
									return component(StatusDot, {
										get colors() {
											return props.colors;
										},
										get tone() {
											return state().sync.running ? "success" : "muted";
										}
									});
								},
								last: true
							});
						}
					})]);
					return __nsh_hv0;
				})(),
				(() => {
					const __nsh_hv1 = element("view");
					insert(__nsh_hv1, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Связанные";
						}
					}), component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return peers().length ? peers().map((device, index, list) => component(CardRow, {
								get key() {
									return device.id;
								},
								get colors() {
									return props.colors;
								},
								icon: "monitor",
								get iconFill() {
									return props.colors.raised;
								},
								get title() {
									return device.name;
								},
								get subtitle() {
									return device.status === "online" ? "Онлайн" : "Не в сети";
								},
								get last() {
									return index === list.length - 1;
								},
								get right() {
									return (() => {
										const __nsh_hv2 = element("view");
										prop(__nsh_hv2, "class", "flex-row items-center gap-2");
										insert(__nsh_hv2, [component(StatusDot, {
											get colors() {
												return props.colors;
											},
											get tone() {
												return device.status === "online" ? "success" : "muted";
											}
										}), component(PillButton, {
											get colors() {
												return props.colors;
											},
											tone: "danger",
											label: "Отключить",
											onPress: () => props.onCommand("device.unlink", { id: device.id })
										})]);
										return __nsh_hv2;
									})();
								}
							})) : component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Пока никого",
								subtitle: "Подключите второе устройство, чтобы делиться задачами и заметками",
								last: true
							});
						}
					})]);
					return __nsh_hv1;
				})(),
				(() => {
					const __nsh_hv3 = element("view");
					insert(__nsh_hv3, component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return component(CardRow, {
								get colors() {
									return props.colors;
								},
								icon: "plus",
								get iconFill() {
									return props.colors.raised;
								},
								title: "Подключить устройство",
								subtitle: "Введите код, который показывает другое устройство",
								last: true,
								get right() {
									return component(PillButton, {
										get colors() {
											return props.colors;
										},
										tone: "accent",
										label: "Подключить",
										onPress: () => props.onCommand("connect.open")
									});
								}
							});
						}
					}));
					return __nsh_hv3;
				})()
			];
		}
	});
}
/** Connectors are the Engine's (`integrations.list`): the Manager shows what it
* reports and can ask for a sync. Which services exist is the Engine's catalog,
* not a list the Manager keeps. */
function IntegrationsPage(props) {
	const connectors = () => props.state().connectors;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Интеграции",
		get children() {
			return [(() => {
				const __nsh_hv4 = element("view");
				insert(__nsh_hv4, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Коннекторы";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return connectors().length ? connectors().map((connector, index, list) => component(CardRow, {
							get key() {
								return connector.id;
							},
							get colors() {
								return props.colors;
							},
							icon: "plug",
							get iconFill() {
								return props.colors.raised;
							},
							get accent() {
								return connector.connected ? props.colors.success : props.colors.muted;
							},
							get title() {
								return connector.name;
							},
							get subtitle() {
								return connector.detail || (connector.connected ? "Подключено" : "Не подключено");
							},
							get last() {
								return index === list.length - 1;
							},
							get right() {
								return component(StatusDot, {
									get colors() {
										return props.colors;
									},
									get tone() {
										return connector.connected ? "success" : "muted";
									}
								});
							}
						})) : component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Коннекторов нет",
							subtitle: "Engine не сообщает о подключённых сервисах",
							last: true
						});
					}
				})]);
				return __nsh_hv4;
			})(), (() => {
				const __nsh_hv5 = element("view");
				insert(__nsh_hv5, component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return [component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "key",
							get iconFill() {
								return props.colors.raised;
							},
							title: "Доступы и ключи",
							subtitle: "Диктовка и распознавание речи",
							get right() {
								return component(PillButton, {
									get colors() {
										return props.colors;
									},
									label: "Открыть",
									onPress: () => props.onCommand("route", { route: "keys" })
								});
							}
						}), component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "pulse",
							get iconFill() {
								return props.colors.raised;
							},
							title: "Синхронизировать сейчас",
							subtitle: "Забрать свежие данные из подключённых сервисов",
							last: true,
							get right() {
								return component(PillButton, {
									get colors() {
										return props.colors;
									},
									tone: "accent",
									label: "Синхронизировать",
									onPress: () => props.onCommand("connectors.syncNow")
								});
							}
						})];
					}
				}));
				return __nsh_hv5;
			})()];
		}
	});
}
/**
* The Manager hosts no browser of its own and the Engine exposes no browser
* settings, so the page states what is true and offers nothing to press: a
* control that never reaches the Engine is worse than no control.
*/
function BrowserPage(props) {
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Браузер",
		get children() {
			return (() => {
				const __nsh_hv6 = element("view");
				insert(__nsh_hv6, [component(SectionLabel, {
					get colors() {
						return props.colors;
					},
					get children() {
						return "Ссылки";
					}
				}), component(Card, {
					get colors() {
						return props.colors;
					},
					get children() {
						return component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "external",
							get iconFill() {
								return props.colors.raised;
							},
							title: "Системный браузер",
							subtitle: "Mundus не держит свой: ссылки открывает тот браузер, который выбран в системе",
							last: true,
							get right() {
								return (() => {
									const __nsh_hv7 = element("text");
									bindProp(__nsh_hv7, "style", () => ({
										fontSize: 12.5,
										color: props.colors.muted
									}));
									const __nsh_hv8 = text("Системный");
									append(__nsh_hv7, __nsh_hv8);
									return __nsh_hv7;
								})();
							}
						});
					}
				})]);
				return __nsh_hv6;
			})();
		}
	});
}

//#endregion
//#region App.tsx
setup(null, "{\n  \"cssVars\": {\n    \"theme\": {\n      \"font-sans\": \"Inter\",\n      \"font-size-body\": \"13px\",\n      \"font-size-label\": \"12px\"\n    }\n  }\n}\n", null, null);
const state = signal(globalThis.__MUNDUS_MANAGER_INITIAL__ ?? demoState);
const search = signal("");
const confirmStop = signal(false);
const confirmInstall = signal(false);
const restoreId = signal("");
const connectOpen = signal(false);
const connectCode = signal("");
const Bridge = nativeComponent("manager.bridge", {
	props: ["route"],
	events: [
		"state",
		"navigate",
		"hotkey",
		"hotkeyState"
	]
});
let session;
/** The palette the pages wear. Mundus keeps one: themes belong to the apps the
* person runs inside it, not to the Manager's own window. */
const colors = () => cobaltFoundry;
function patch(next) {
	state.set({
		...state(),
		...next
	});
}
/** Sends an Engine operation. Without a host the call goes nowhere, and the
* pages keep their demo values so the interface still reads in `nsh dev`. */
function dispatch(name, args = {}) {
	try {
		session?.command(name, args);
	} catch {}
}
/**
* What a press means. Irreversible Engine operations ask first, and everything
* else is a plain call.
*/
function command(name, args = {}) {
	switch (name) {
		case "server.stop":
			confirmStop.set(true);
			return;
		case "updates.install":
			confirmInstall.set(true);
			return;
		case "backups.restore":
			restoreId.set(String(args.id ?? ""));
			return;
		case "connect.open":
			connectOpen.set(true);
			return;
		case "pairing.hide":
			patch({ sync: {
				...state().sync,
				ownCode: ""
			} });
			return;
		case "route":
			go(String(args.route));
			return;
		default: dispatch(name, args);
	}
}
/**
* A window with no system bar of its own: the first row of the content is the
* band the caption buttons sit in, and `titleBar()` reports the caption area so
* the sidebar can inset below it.
*/
function useHiddenChrome() {
	effect(() => Window.setTitleBar({
		style: "hidden",
		height: 36
	}));
}
function renderPage(page) {
	const props = {
		colors: colors(),
		state: () => state(),
		onCommand: command
	};
	switch (page) {
		case "plugins": return component(PluginsPage, props);
		case "integrations": return component(IntegrationsPage, props);
		case "storage": return component(StoragePage, props);
		case "activity": return component(ActivityPage, props);
		case "devices": return component(DevicesPage, props);
		case "browser": return component(BrowserPage, props);
		case "keys": return component(KeysPage, props);
		case "models": return component(ModelsPage, props);
		case "settings": return component(PreferencesPage, props);
		default: {
			const id = page.startsWith("plugin/") ? page.slice(7) : "";
			if (id) return component(PluginPage, merge(props, { id }));
			return component(ServerPage, props);
		}
	}
}
/**
* The sidebar: the product's own groups, then the plugins that are installed.
* A plugin is not a page the Manager was built around — it sits below the
* machine's settings because it is the person's addition, and it disappears
* with the plugin.
*/
function sidebarGroups() {
	const installed = state().plugins.installed;
	const status = state().engine.status;
	const caption = status === "running" ? "Работает" : status === "starting" ? "Запускается" : status === "stopping" ? "Останавливается" : "Не отвечает";
	const groups = navigation.map((group) => ({
		...group,
		entries: group.entries.map((entry) => entry.route === "server" ? {
			...entry,
			caption
		} : entry)
	}));
	if (!installed.length) return groups;
	return [...groups, {
		id: "plugins-installed",
		entries: installed.map((plugin) => ({
			route: `plugin/${plugin.id}`,
			label: plugin.name,
			icon: "plug",
			accent: "#15b8a4",
			accentTo: "#1784fc"
		}))
	}];
}
/**
* The pairing request the Engine is holding, if there is one to answer. The
* Engine is the one that closes the handshake: the modal leaves only once the
* poller reports the request gone, so a failed call keeps it on screen.
*/
function pendingPairing() {
	return state().sync.incoming[0];
}
function PairingModal(props) {
	const request = pendingPairing();
	if (!request) return null;
	const answer = (name) => dispatch(name, { id: request.id });
	const who = request.platform && request.appVersion ? `${request.platform} · ${request.appVersion}` : void 0;
	return component(Modal, {
		title: "Запрос на подключение",
		get colors() {
			return props.colors;
		},
		onDismiss: () => answer("sync.decline"),
		get footer() {
			return [component(Cancel, {
				get colors() {
					return props.colors;
				},
				label: "Отклонить",
				onPress: () => answer("sync.decline")
			}), component(Confirm, {
				get colors() {
					return props.colors;
				},
				label: "Принять",
				onPress: () => answer("sync.accept")
			})];
		},
		get children() {
			return [
				(() => {
					const __nsh_hv0 = element("text");
					bindProp(__nsh_hv0, "style", () => ({
						fontSize: 13,
						lineHeight: 19,
						color: props.colors.foreground
					}));
					insert(__nsh_hv0, () => `«${request.name}» хочет подключиться к этому компьютеру.`);
					return __nsh_hv0;
				})(),
				() => who ? note(props.colors, who) : null,
				() => note(props.colors, "Устройство получит доступ к задачам, заметкам и истории. Подключайте только то, что узнаёте.")
			];
		}
	});
}
function App() {
	useHiddenChrome();
	return (() => {
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "class", "h-full w-full");
		bindProp(__nsh_hv1, "style", () => ({ backgroundColor: colors().background }));
		const __nsh_hv2 = element("view");
		prop(__nsh_hv2, "class", "min-h-0 flex-1 flex-row");
		const __nsh_hv3 = element("view");
		prop(__nsh_hv3, "class", "h-full relative overflow-hidden");
		bindProp(__nsh_hv3, "style", () => ({
			width: settingsMetrics.sidebarWidth,
			backgroundColor: colors().shell
		}));
		spread(__nsh_hv3, dragBinding());
		const __nsh_hv4 = element("view");
		prop(__nsh_hv4, "class", "absolute inset-0");
		spread(__nsh_hv4, dragBinding());
		append(__nsh_hv3, __nsh_hv4);
		insert(__nsh_hv3, component(Sidebar, {
			get colors() {
				return colors();
			},
			get groups() {
				return sidebarGroups();
			},
			get current() {
				return currentRoute();
			},
			get search() {
				return search();
			},
			get topInset() {
				return sidebarTopInset(settingsMetrics.searchHeight);
			},
			onSearch: (value) => search.set(value),
			onNavigate: (next) => go(next)
		}));
		append(__nsh_hv2, __nsh_hv3);
		insert(__nsh_hv2, () => renderPage(currentRoute()));
		append(__nsh_hv1, __nsh_hv2);
		insert(__nsh_hv1, component(Bridge, {
			ref: (node) => {
				session = { command: (name, args) => node.command(name, args) };
				connectHost((name) => node.command(name, {}));
			},
			get route() {
				return currentRoute();
			},
			onState: (next) => state.set(next),
			onNavigate: (event) => go(event.route),
			onHotkey: (event) => {
				if (event.accelerator) hotkeyCaptured.set(event.accelerator);
			},
			onHotkeyState: (event) => {
				const now = state();
				state.set({
					...now,
					hotkey: {
						...now.hotkey,
						...event
					}
				});
			}
		}), __nsh_hv2);
		insert(__nsh_hv1, [
			component(PairingModal, { get colors() {
				return colors();
			} }),
			() => confirmStop() ? component(Modal, {
				title: "Остановить Mundus?",
				get colors() {
					return colors();
				},
				onDismiss: () => confirmStop.set(false),
				get footer() {
					return [component(Cancel, {
						get colors() {
							return colors();
						},
						onPress: () => confirmStop.set(false)
					}), component(Confirm, {
						get colors() {
							return colors();
						},
						tone: "danger",
						label: "Остановить",
						onPress: () => {
							confirmStop.set(false);
							dispatch("server.stop");
						}
					})];
				},
				get children() {
					return note(colors(), "Atlas и диктовка потеряют связь с этим компьютером до следующего запуска. Данные останутся на месте.");
				}
			}) : null,
			() => confirmInstall() ? component(Modal, {
				title: "Установить обновление?",
				get colors() {
					return colors();
				},
				onDismiss: () => confirmInstall.set(false),
				get footer() {
					return [component(Cancel, {
						get colors() {
							return colors();
						},
						onPress: () => confirmInstall.set(false)
					}), component(Confirm, {
						get colors() {
							return colors();
						},
						label: "Установить",
						onPress: () => {
							confirmInstall.set(false);
							dispatch("updates.install");
						}
					})];
				},
				get children() {
					return note(colors(), "Mundus закроется, поставит новую версию и запустится снова. Это займёт около минуты.");
				}
			}) : null,
			() => restoreId() ? component(Modal, {
				title: "Восстановить копию?",
				get colors() {
					return colors();
				},
				onDismiss: () => restoreId.set(""),
				get footer() {
					return [component(Cancel, {
						get colors() {
							return colors();
						},
						onPress: () => restoreId.set("")
					}), component(Confirm, {
						get colors() {
							return colors();
						},
						tone: "danger",
						label: "Восстановить",
						onPress: () => {
							const id = restoreId();
							restoreId.set("");
							dispatch("backups.restore", { id });
						}
					})];
				},
				get children() {
					return note(colors(), `База заменится копией ${restoreId()}. Всё, что записано после неё, пропадёт.`);
				}
			}) : null,
			() => connectOpen() ? component(Modal, {
				title: "Подключить устройство",
				get colors() {
					return colors();
				},
				onDismiss: () => connectOpen.set(false),
				get footer() {
					return [component(Cancel, {
						get colors() {
							return colors();
						},
						onPress: () => connectOpen.set(false)
					}), component(Confirm, {
						get colors() {
							return colors();
						},
						label: "Подключить",
						onPress: () => {
							const code = connectCode();
							connectOpen.set(false);
							connectCode.set("");
							dispatch("connect.code", { value: code });
						}
					})];
				},
				get children() {
					return [() => note(colors(), "Откройте Mundus на другом устройстве, покажите там код и введите его здесь."), component(TextInput, {
						get colors() {
							return colors();
						},
						get value() {
							return connectCode();
						},
						placeholder: "Код подключения",
						accessibilityLabel: "Код подключения",
						onChange: (value) => connectCode.set(value)
					})];
				}
			}) : null
		]);
		return __nsh_hv1;
	})();
}
mount(App);

//#endregion