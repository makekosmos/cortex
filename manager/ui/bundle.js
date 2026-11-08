import { append, bindProp, component, element, insert, merge, prop, setup, text } from "nsh:rt";
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
/** A 24pt gradient tile with a white glyph — the sidebar's colour language. */
function IconTile(props) {
	const size = props.size ?? settingsMetrics.tileSize;
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "class", "items-center justify-center");
		bindProp(__nsh_hv0, "style", () => ({
			width: size,
			height: size,
			borderRadius: props.radius ?? settingsMetrics.tileRadius,
			backgroundImage: tileGradient(props.accent, props.accentTo),
			borderWidth: 1,
			borderColor: "rgba(255,255,255,0.20)",
			boxShadow: "0 1px 2px rgba(0,0,0,0.32)"
		}));
		insert(__nsh_hv0, component(Icon, {
			get name() {
				return props.icon;
			},
			get size() {
				return Math.round(size * .72);
			},
			color: "#ffffff"
		}));
		return __nsh_hv0;
	})();
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
/** Sidebar row: accent tile, label and an optional caption under it. */
function NavTile(props) {
	return (() => {
		const __nsh_hv2 = element("view");
		bindProp(__nsh_hv2, "onPress", () => props.onPress);
		prop(__nsh_hv2, "role", "button");
		bindProp(__nsh_hv2, "accessibilityLabel", () => props.label);
		bindProp(__nsh_hv2, "accessibilityState", () => ({ selected: props.selected }));
		prop(__nsh_hv2, "class", "flex-row items-center gap-2.5 rounded-[10px]");
		bindProp(__nsh_hv2, "style", () => ({
			height: props.logo ? settingsMetrics.logoRowHeight : settingsMetrics.navRowHeight,
			paddingLeft: settingsMetrics.cardPaddingX,
			paddingRight: settingsMetrics.cardPaddingX,
			backgroundColor: props.selected ? props.colors.raised : "transparent"
		}));
		const __nsh_hv3 = element("view");
		bindProp(__nsh_hv3, "style", () => ({ marginTop: props.logo ? 0 : -1 }));
		insert(__nsh_hv3, () => props.logo ? component(LogoTile, {}) : component(IconTile, {
			get icon() {
				return props.icon;
			},
			get accent() {
				return props.accent;
			},
			get accentTo() {
				return props.accentTo;
			}
		}));
		const __nsh_hv4 = element("view");
		prop(__nsh_hv4, "class", "flex-1");
		prop(__nsh_hv4, "style", { gap: 4 });
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
function SettingsSidebar(props) {
	const matches = (entry) => !props.search.trim() || entry.label.toLowerCase().includes(props.search.trim().toLowerCase());
	const lights = titleBar();
	const top = lights.left > 0 ? lights.height + 4 : settingsMetrics.sidebarTop;
	return (() => {
		const __nsh_hv9 = element("view");
		prop(__nsh_hv9, "class", "h-full shrink-0 flex-col");
		bindProp(__nsh_hv9, "style", () => ({
			width: settingsMetrics.sidebarWidth,
			backgroundColor: props.colors.shell,
			paddingTop: top,
			paddingBottom: settingsMetrics.sidebarTop,
			paddingLeft: settingsMetrics.sidebarPadding,
			paddingRight: settingsMetrics.sidebarPadding
		}));
		const __nsh_hv10 = element("view");
		prop(__nsh_hv10, "class", "min-h-0 flex-1 flex-col");
		bindProp(__nsh_hv10, "style", () => ({ gap: settingsMetrics.searchGap }));
		const __nsh_hv11 = element("scroll");
		prop(__nsh_hv11, "class", "min-h-0 flex-1");
		const __nsh_hv12 = element("view");
		bindProp(__nsh_hv12, "style", () => ({ gap: settingsMetrics.navGroupGap }));
		insert(__nsh_hv12, () => props.groups.map((group, index) => {
			const entries = group.entries.filter(matches);
			if (!entries.length) return null;
			return (() => {
				const __nsh_hv13 = element("view");
				bindProp(__nsh_hv13, "key", () => group.id);
				bindProp(__nsh_hv13, "style", () => ({
					gap: settingsMetrics.navEntryGap,
					marginBottom: index === 0 ? -1 : 0
				}));
				insert(__nsh_hv13, () => entries.map((entry) => component(NavTile, {
					get key() {
						return entry.route;
					},
					get label() {
						return entry.label;
					},
					get icon() {
						return entry.icon;
					},
					get accent() {
						return entry.accent;
					},
					get accentTo() {
						return entry.accentTo;
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
				return __nsh_hv13;
			})();
		}));
		append(__nsh_hv11, __nsh_hv12);
		append(__nsh_hv10, __nsh_hv11);
		insert(__nsh_hv10, component(SearchField, {
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
		}), __nsh_hv11);
		append(__nsh_hv9, __nsh_hv10);
		return __nsh_hv9;
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
/** A theme preview: the palette painted as a mini app, plus swatch circles. */
function ThemeCard(props) {
	const swatches = [
		props.colors.shell,
		props.colors.danger,
		"#c9b45a",
		"#7eae4d",
		props.colors.success,
		"#3aa6b9",
		props.colors.accent,
		"#6f7ff2",
		"#b07cff",
		"#d970b8",
		props.colors.muted
	];
	return (() => {
		const __nsh_hv38 = element("view");
		bindProp(__nsh_hv38, "onPress", () => props.onPress);
		prop(__nsh_hv38, "role", "button");
		bindProp(__nsh_hv38, "accessibilityLabel", () => props.name);
		bindProp(__nsh_hv38, "accessibilityState", () => ({ selected: props.selected }));
		prop(__nsh_hv38, "style", {
			width: 182,
			gap: 9
		});
		const __nsh_hv39 = element("view");
		prop(__nsh_hv39, "class", "w-full overflow-hidden");
		bindProp(__nsh_hv39, "style", () => ({
			height: 74,
			borderRadius: 10,
			borderWidth: props.selected ? 2 : 1,
			borderColor: props.selected ? props.colors.accent : props.colors.border,
			backgroundColor: props.colors.background,
			paddingLeft: 11,
			paddingRight: 11,
			paddingTop: 10,
			paddingBottom: 9,
			justifyContent: "space-between"
		}));
		const __nsh_hv40 = element("view");
		prop(__nsh_hv40, "class", "flex-row items-center justify-between");
		const __nsh_hv41 = element("text");
		bindProp(__nsh_hv41, "style", () => ({
			fontSize: 16,
			fontWeight: "600",
			color: props.colors.foreground
		}));
		const __nsh_hv42 = text("Aa");
		append(__nsh_hv41, __nsh_hv42);
		const __nsh_hv43 = element("view");
		prop(__nsh_hv43, "class", "flex-row items-center");
		prop(__nsh_hv43, "style", { gap: 5 });
		const __nsh_hv44 = element("view");
		prop(__nsh_hv44, "class", "items-center justify-center");
		bindProp(__nsh_hv44, "style", () => ({
			height: 19,
			paddingLeft: 5,
			paddingRight: 5,
			borderRadius: 5,
			backgroundColor: props.colors.dangerFill
		}));
		const __nsh_hv45 = element("text");
		bindProp(__nsh_hv45, "style", () => ({
			fontSize: 10,
			color: props.colors.danger
		}));
		const __nsh_hv46 = text("~/");
		append(__nsh_hv45, __nsh_hv46);
		append(__nsh_hv44, __nsh_hv45);
		const __nsh_hv47 = element("view");
		bindProp(__nsh_hv47, "style", () => ({
			width: 11,
			height: 19,
			borderRadius: 5,
			backgroundColor: props.colors.accent
		}));
		append(__nsh_hv43, __nsh_hv44);
		append(__nsh_hv43, __nsh_hv47);
		append(__nsh_hv40, __nsh_hv41);
		append(__nsh_hv40, __nsh_hv43);
		const __nsh_hv48 = element("view");
		prop(__nsh_hv48, "class", "flex-row items-center");
		prop(__nsh_hv48, "style", { gap: 4 });
		insert(__nsh_hv48, () => swatches.map((swatch, index) => (() => {
			const __nsh_hv49 = element("view");
			prop(__nsh_hv49, "key", `${swatch}-${index}`);
			prop(__nsh_hv49, "style", {
				width: 9,
				height: 9,
				borderRadius: 5,
				backgroundColor: swatch
			});
			return __nsh_hv49;
		})()));
		append(__nsh_hv39, __nsh_hv40);
		append(__nsh_hv39, __nsh_hv48);
		const __nsh_hv50 = element("text");
		bindProp(__nsh_hv50, "style", () => ({
			fontSize: 12,
			fontWeight: props.selected ? "600" : "500",
			color: props.captionColor ?? (props.selected ? props.colors.foreground : props.colors.muted)
		}));
		insert(__nsh_hv50, () => props.name);
		append(__nsh_hv38, __nsh_hv39);
		append(__nsh_hv38, __nsh_hv50);
		return __nsh_hv38;
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
		id: "app",
		entries: [{
			route: "about",
			label: "О приложении",
			icon: "info",
			accent: "#1784fc",
			accentTo: "#915bfc"
		}, {
			route: "appearance",
			label: "Внешний вид",
			icon: "palette",
			accent: "#f79411",
			accentTo: "spectrum"
		}]
	},
	{
		id: "running",
		entries: [
			{
				route: "apps",
				label: "Приложения",
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
//#region state.ts
const demoState = {
	engine: {
		status: "running",
		version: "0.10.26",
		channel: "production",
		port: 47321
	},
	client: { name: "Этот компьютер" },
	product: { bundleId: "com.kazui.mundus.manager" },
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
		size: "1.8 ГБ"
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
	},
	appearance: {
		mode: "dark",
		light: "merino-light",
		dark: "cobalt-foundry",
		followApps: false,
		accentSource: "theme"
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
		const __nsh_hv0 = element("scroll");
		prop(__nsh_hv0, "class", "min-h-0 flex-1");
		bindProp(__nsh_hv0, "style", () => ({ backgroundColor: props.colors.background }));
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "class", "w-full");
		bindProp(__nsh_hv1, "style", () => ({
			paddingLeft: settingsMetrics.pageGutter,
			paddingRight: settingsMetrics.pageGutter,
			paddingTop: settingsMetrics.pageTop,
			paddingBottom: settingsMetrics.pageBottom
		}));
		const __nsh_hv2 = element("view");
		prop(__nsh_hv2, "class", "flex-row items-center");
		bindProp(__nsh_hv2, "style", () => ({
			height: settingsMetrics.headerHeight,
			paddingLeft: controls.left
		}));
		prop(__nsh_hv2, "desktop", { windowDrag: true });
		const __nsh_hv4 = element("view");
		prop(__nsh_hv4, "class", "flex-1");
		append(__nsh_hv2, __nsh_hv4);
		insert(__nsh_hv2, [() => props.title ? (() => {
			const __nsh_hv3 = element("text");
			bindProp(__nsh_hv3, "style", () => ({
				fontSize: 13,
				lineHeight: 19,
				fontWeight: "500",
				color: props.colors.foreground
			}));
			insert(__nsh_hv3, () => props.title);
			return __nsh_hv3;
		})() : null, () => props.actions ?? null], __nsh_hv4);
		const __nsh_hv5 = element("view");
		bindProp(__nsh_hv5, "style", () => ({
			gap: settingsMetrics.sectionGap,
			marginTop: settingsMetrics.headerGap
		}));
		insert(__nsh_hv5, () => props.children);
		append(__nsh_hv1, __nsh_hv2);
		append(__nsh_hv1, __nsh_hv5);
		append(__nsh_hv0, __nsh_hv1);
		return __nsh_hv0;
	})();
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
							title: "Mundus",
							get subtitle() {
								return statusLabel(state().engine.status);
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
											const __nsh_hv1 = element("text");
											bindProp(__nsh_hv1, "style", () => ({
												fontSize: 11.5,
												color: props.colors.muted
											}));
											insert(__nsh_hv1, code);
											return __nsh_hv1;
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
					return __nsh_hv0;
				})(),
				(() => {
					const __nsh_hv2 = element("view");
					insert(__nsh_hv2, [component(SectionLabel, {
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
										const __nsh_hv3 = element("text");
										bindProp(__nsh_hv3, "style", () => ({
											fontSize: 13,
											color: props.colors.foreground
										}));
										insert(__nsh_hv3, () => state().client.name);
										return __nsh_hv3;
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
										const __nsh_hv4 = element("text");
										bindProp(__nsh_hv4, "style", () => ({
											fontSize: 12.5,
											color: props.colors.muted
										}));
										insert(__nsh_hv4, () => state().engine.port || "—");
										return __nsh_hv4;
									})();
								}
							})];
						}
					})]);
					return __nsh_hv2;
				})()
			];
		}
	});
}

//#endregion
//#region pages/appearance.tsx
function ThemeRow(props) {
	return (() => {
		const __nsh_hv0 = element("view");
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "class", "flex-row flex-wrap");
		prop(__nsh_hv1, "style", { gap: 14 });
		insert(__nsh_hv1, () => props.themes.map((theme) => component(ThemeCard, {
			get key() {
				return theme.key;
			},
			get name() {
				return theme.name;
			},
			get colors() {
				return theme.colors;
			},
			get selected() {
				return props.selected === theme.key;
			},
			get active() {
				return props.selected === theme.key;
			},
			get captionColor() {
				return props.selected === theme.key ? props.colors.foreground : props.colors.muted;
			},
			onPress: () => props.onPick(theme.key)
		})));
		append(__nsh_hv0, __nsh_hv1);
		insert(__nsh_hv0, component(SectionLabel, {
			get colors() {
				return props.colors;
			},
			get children() {
				return props.title;
			}
		}), __nsh_hv1);
		return __nsh_hv0;
	})();
}
/**
* Themes and palette. The choice is device-wide and lives in the Engine
* (`appearance.get`/`appearance.set`), so every Mundus window on this computer
* wears the same one — the pages only read it and ask for a change.
*/
function AppearancePage(props) {
	const state = () => props.state();
	const follow = () => state().appearance.followApps;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Внешний вид",
		get children() {
			return [
				component(ThemeRow, {
					get title() {
						return `Светлая тема${state().appearance.mode === "light" ? " — активна" : ""}`;
					},
					themes: lightThemes,
					get selected() {
						return state().appearance.light;
					},
					get colors() {
						return props.colors;
					},
					onPick: (key) => props.onCommand("appearance.theme", {
						mode: "light",
						key
					})
				}),
				component(ThemeRow, {
					get title() {
						return `Тёмная тема${state().appearance.mode === "dark" ? " — активна" : ""}`;
					},
					themes: darkThemes,
					get selected() {
						return state().appearance.dark;
					},
					get colors() {
						return props.colors;
					},
					onPick: (key) => props.onCommand("appearance.theme", {
						mode: "dark",
						key
					})
				}),
				(() => {
					const __nsh_hv2 = element("view");
					insert(__nsh_hv2, component(Card, {
						get colors() {
							return props.colors;
						},
						get children() {
							return component(CardRow, {
								get colors() {
									return props.colors;
								},
								title: "Следовать теме приложений",
								get subtitle() {
									return follow() ? "Mundus берёт светлую или тёмную тему из системы" : "Тема выбирается здесь, независимо от системы";
								},
								last: true,
								get right() {
									return component(Switch, {
										get colors() {
											return props.colors;
										},
										get value() {
											return follow();
										},
										accessibilityLabel: "Следовать теме приложений",
										onChange: (value) => props.onCommand("appearance.followApps", { value })
									});
								}
							});
						}
					}));
					return __nsh_hv2;
				})(),
				(() => {
					const __nsh_hv3 = element("view");
					insert(__nsh_hv3, [component(SectionLabel, {
						get colors() {
							return props.colors;
						},
						get children() {
							return "Интерфейс";
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
								title: "Акцентный цвет",
								get subtitle() {
									return state().appearance.accentSource === "wallpaper" ? "Взят с обоев" : "Взят из темы";
								},
								get right() {
									return (() => {
										const __nsh_hv4 = element("view");
										bindProp(__nsh_hv4, "style", () => ({
											width: 26,
											height: 26,
											borderRadius: 13,
											backgroundColor: props.colors.accent
										}));
										return __nsh_hv4;
									})();
								},
								last: true
							});
						}
					})]);
					return __nsh_hv3;
				})(),
				(() => {
					const __nsh_hv5 = element("view");
					prop(__nsh_hv5, "class", "flex-row items-center gap-1.5");
					const __nsh_hv6 = element("text");
					bindProp(__nsh_hv6, "style", () => ({
						fontSize: 12,
						color: props.colors.faint
					}));
					const __nsh_hv7 = text("Тема применяется ко всем окнам Mundus на этом компьютере.");
					append(__nsh_hv6, __nsh_hv7);
					append(__nsh_hv5, __nsh_hv6);
					insert(__nsh_hv5, component(Icon, {
						name: "palette",
						size: 13,
						get color() {
							return props.colors.faint;
						}
					}), __nsh_hv6);
					return __nsh_hv5;
				})()
			];
		}
	});
}

//#endregion
//#region pages/system.tsx
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
/** The update card's one button: check, download, or install. Nothing is
* downloaded without this press. */
function UpdateAction(props) {
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
	if (updates.canInstall) return `Версия ${updates.newVersion} готова к установке`;
	if (updates.newVersion) return `Доступна версия ${updates.newVersion}`;
	if (updates.state === "checking") return "Проверяем…";
	if (updates.state === "error") return "Не удалось проверить обновления";
	return "Установлена последняя версия";
}
function AboutPage(props) {
	const state = () => props.state();
	const version = productVersion(state());
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "О приложении",
		get children() {
			return [component(Card, {
				get colors() {
					return props.colors;
				},
				get children() {
					return [
						component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "server",
							get iconFill() {
								return props.colors.raised;
							},
							get accent() {
								return props.colors.accent;
							},
							title: version ? `Mundus ${version}` : "Mundus",
							get subtitle() {
								return version ? `канал ${state().engine.channel}` : statusLabel(state().engine.status);
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
							}
						}),
						component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Engine",
							get subtitle() {
								return state().engine.version ? `${statusLabel(state().engine.status)} · ${state().engine.version}` : statusLabel(state().engine.status);
							},
							get right() {
								return state().engine.port ? (() => {
									const __nsh_hv1 = element("text");
									bindProp(__nsh_hv1, "style", () => ({
										fontSize: 12.5,
										color: props.colors.muted
									}));
									const __nsh_hv2 = text("порт ");
									append(__nsh_hv1, __nsh_hv2);
									insert(__nsh_hv1, () => state().engine.port);
									return __nsh_hv1;
								})() : null;
							}
						}),
						component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Обновления",
							get subtitle() {
								return updateSubtitle(state().updates);
							},
							get right() {
								return component(UpdateAction, merge(props, { get updates() {
									return state().updates;
								} }));
							}
						}),
						component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Mundus Manager",
							get subtitle() {
								return state().product.bundleId ?? "";
							},
							get right() {
								return (() => {
									const __nsh_hv3 = element("text");
									bindProp(__nsh_hv3, "style", () => ({
										fontSize: 12.5,
										color: props.colors.muted
									}));
									const __nsh_hv4 = text("менеджер");
									append(__nsh_hv3, __nsh_hv4);
									return __nsh_hv3;
								})();
							},
							last: true
						})
					];
				}
			}), (() => {
				const __nsh_hv5 = element("text");
				bindProp(__nsh_hv5, "style", () => ({
					fontSize: 12,
					color: props.colors.faint
				}));
				const __nsh_hv6 = text("© Yoso Industries 2025–2026");
				append(__nsh_hv5, __nsh_hv6);
				return __nsh_hv5;
			})()];
		}
	});
}
function StoragePage(props) {
	const state = () => props.state();
	const backups = () => state().backups;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Хранилище",
		get children() {
			return [(() => {
				const __nsh_hv7 = element("view");
				insert(__nsh_hv7, [component(SectionLabel, {
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
						return [component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Папка данных",
							get subtitle() {
								return state().storage.dataDir;
							}
						}), component(CardRow, {
							get colors() {
								return props.colors;
							},
							title: "Размер базы",
							get subtitle() {
								return state().storage.size || "неизвестен";
							},
							last: true
						})];
					}
				})]);
				return __nsh_hv7;
			})(), (() => {
				const __nsh_hv8 = element("view");
				insert(__nsh_hv8, [component(SectionLabel, {
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
						return [() => backups().length ? backups().map((backup, index, list) => component(CardRow, {
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
				return __nsh_hv8;
			})()];
		}
	});
}
function ActivityPage(props) {
	const usage = () => props.state().usage;
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Активность",
		get children() {
			return [(() => {
				const __nsh_hv9 = element("view");
				insert(__nsh_hv9, [component(SectionLabel, {
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
				return __nsh_hv9;
			})(), (() => {
				const __nsh_hv10 = element("view");
				insert(__nsh_hv10, [component(SectionLabel, {
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
							subtitle: "Учёт активности выключен или пуст",
							last: true
						});
					}
				})]);
				return __nsh_hv10;
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
				const __nsh_hv11 = element("view");
				insert(__nsh_hv11, [component(SectionLabel, {
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
				return __nsh_hv11;
			})();
		}
	});
}

//#endregion
//#region pages/library.tsx
/** The Engine of this branch keeps no installed-app catalog, and the Start-Menu
* index it does keep is not one: it names programs on the system, not apps
* Mundus installed. So the page says that much and offers nothing to press —
* a button that opens what it cannot name is worse than no button. */
function AppsPage(props) {
	return component(Page, {
		get colors() {
			return props.colors;
		},
		title: "Приложения",
		get children() {
			return (() => {
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
						return component(CardRow, {
							get colors() {
								return props.colors;
							},
							icon: "apps",
							get iconFill() {
								return props.colors.raised;
							},
							get accent() {
								return props.colors.muted;
							},
							title: "Пока ничего не установлено",
							subtitle: "Engine не сообщает об установленных приложениях",
							last: true
						});
					}
				})]);
				return __nsh_hv0;
			})();
		}
	});
}
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
				const __nsh_hv1 = element("view");
				insert(__nsh_hv1, [component(SectionLabel, {
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
									const __nsh_hv2 = element("view");
									prop(__nsh_hv2, "class", "flex-row items-center gap-2");
									insert(__nsh_hv2, () => model.downloading ? (() => {
										const __nsh_hv3 = element("text");
										bindProp(__nsh_hv3, "style", () => ({
											fontSize: 12.5,
											color: props.colors.muted
										}));
										insert(__nsh_hv3, () => `${Math.round(model.percent)}%`);
										return __nsh_hv3;
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
									return __nsh_hv2;
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
				return __nsh_hv1;
			})(), (() => {
				const __nsh_hv4 = element("text");
				bindProp(__nsh_hv4, "style", () => ({
					fontSize: 12,
					color: props.colors.faint
				}));
				const __nsh_hv5 = text("Модели распознавания живут на этом компьютере: звук не уходит в сеть.");
				append(__nsh_hv4, __nsh_hv5);
				return __nsh_hv4;
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
					const __nsh_hv6 = element("view");
					insert(__nsh_hv6, [component(SectionLabel, {
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
					return __nsh_hv6;
				})(),
				(() => {
					const __nsh_hv7 = element("view");
					insert(__nsh_hv7, [component(SectionLabel, {
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
								const __nsh_hv8 = element("view");
								prop(__nsh_hv8, "style", { gap: 10 });
								const __nsh_hv9 = element("view");
								prop(__nsh_hv9, "class", "flex-row items-center justify-end gap-2");
								insert(__nsh_hv9, [component(PillButton, {
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
								append(__nsh_hv8, __nsh_hv9);
								insert(__nsh_hv8, component(TextInput, {
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
								}), __nsh_hv9);
								return __nsh_hv8;
							})();
						}
					})]);
					return __nsh_hv7;
				})(),
				(() => {
					const __nsh_hv10 = element("text");
					bindProp(__nsh_hv10, "style", () => ({
						fontSize: 12,
						color: props.colors.faint
					}));
					const __nsh_hv11 = text("Ключ хранится в связке ключей Engine. Manager его не читает и нигде не показывает.");
					append(__nsh_hv10, __nsh_hv11);
					return __nsh_hv10;
				})()
			];
		}
	});
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
const route = signal(globalThis.__MUNDUS_MANAGER_ROUTE__ ?? "server");
const search = signal("");
const confirmStop = signal(false);
const confirmInstall = signal(false);
const restoreId = signal("");
const connectOpen = signal(false);
const connectCode = signal("");
const Bridge = nativeComponent("manager.bridge", {
	props: ["route"],
	events: ["state", "navigate"]
});
let session;
/** The palette the pages wear: whichever pair of themes the Engine says is on.
* A light mode must not paint a dark card set, or the pages would say «светлая
* тема» over dark cards. */
const colors = () => {
	const { mode, light, dark } = state().appearance;
	return (mode === "light" ? lightThemes : darkThemes).find((theme) => theme.key === (mode === "light" ? light : dark))?.colors ?? cobaltFoundry;
};
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
* What a press means. Irreversible Engine operations ask first, appearance is
* a device-wide setting on the Engine, and everything else is a plain call.
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
		case "appearance.theme":
			dispatch("appearance.set", { patch: args.mode === "light" ? { light_theme: args.key } : { dark_theme: args.key } });
			patch({ appearance: {
				...state().appearance,
				[args.mode === "light" ? "light" : "dark"]: String(args.key)
			} });
			return;
		case "pairing.hide":
			patch({ sync: {
				...state().sync,
				ownCode: ""
			} });
			return;
		case "appearance.followApps":
			dispatch("appearance.set", { patch: { follow_apps: Boolean(args.value) } });
			patch({ appearance: {
				...state().appearance,
				followApps: Boolean(args.value)
			} });
			return;
		case "route":
			route.set(String(args.route));
			return;
		default: dispatch(name, args);
	}
}
/**
* A window with no system bar of its own: the page heading is the top row, and
* `titleBar()` reports the caption area so the sidebar can inset below it.
*/
const TITLEBAR_HEIGHT = 36;
function useHiddenChrome() {
	effect(() => Window.setTitleBar({
		style: "hidden",
		height: TITLEBAR_HEIGHT
	}));
}
/**
* Whether the strip on the right of the band belongs to the window's caption
* buttons. On Windows it always does: an embedded host's buttons are drawn by
* `manager/src/chrome.rs`, and a host that owns its window (`nsh run`, `nsh
* dev`) draws them itself and reports the strip in `titleBar()`. Either way the
* band must keep that strip out of its drag area, or a press on a button starts
* a window move and the button never sees it.
*/
function reservesCaptions() {
	const host = platform();
	return host === "win32" || host === "windows";
}
function TitleBar(props) {
	const macos = platform() === "darwin";
	return (() => {
		const __nsh_hv0 = element("view");
		prop(__nsh_hv0, "class", "w-full shrink-0 flex-row");
		prop(__nsh_hv0, "style", { height: TITLEBAR_HEIGHT });
		const __nsh_hv1 = element("view");
		prop(__nsh_hv1, "desktop", { windowDrag: true });
		prop(__nsh_hv1, "class", "h-full flex-row items-center");
		bindProp(__nsh_hv1, "style", () => ({
			width: settingsMetrics.sidebarWidth,
			backgroundColor: props.colors.shell,
			paddingLeft: macos ? 80 : 14
		}));
		const __nsh_hv2 = element("text");
		bindProp(__nsh_hv2, "style", () => ({
			fontSize: 12,
			fontWeight: "500",
			color: props.colors.muted
		}));
		const __nsh_hv3 = text("Mundus");
		append(__nsh_hv2, __nsh_hv3);
		append(__nsh_hv1, __nsh_hv2);
		const __nsh_hv4 = element("view");
		prop(__nsh_hv4, "desktop", { windowDrag: true });
		prop(__nsh_hv4, "class", "h-full flex-1");
		bindProp(__nsh_hv4, "style", () => ({ backgroundColor: props.colors.background }));
		append(__nsh_hv0, __nsh_hv1);
		append(__nsh_hv0, __nsh_hv4);
		insert(__nsh_hv0, () => reservesCaptions() ? (() => {
			const __nsh_hv5 = element("view");
			bindProp(__nsh_hv5, "style", () => ({
				width: 138,
				height: TITLEBAR_HEIGHT,
				backgroundColor: props.colors.background
			}));
			return __nsh_hv5;
		})() : null);
		return __nsh_hv0;
	})();
}
function renderPage(page) {
	const props = {
		colors: colors(),
		state: () => state(),
		onCommand: command
	};
	switch (page) {
		case "about": return component(AboutPage, props);
		case "appearance": return component(AppearancePage, props);
		case "apps": return component(AppsPage, props);
		case "integrations": return component(IntegrationsPage, props);
		case "storage": return component(StoragePage, props);
		case "activity": return component(ActivityPage, props);
		case "devices": return component(DevicesPage, props);
		case "browser": return component(BrowserPage, props);
		case "keys": return component(KeysPage, props);
		case "models": return component(ModelsPage, props);
		case "settings": return component(PreferencesPage, props);
		default: return component(ServerPage, props);
	}
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
					const __nsh_hv6 = element("text");
					bindProp(__nsh_hv6, "style", () => ({
						fontSize: 13,
						lineHeight: 19,
						color: props.colors.foreground
					}));
					insert(__nsh_hv6, () => `«${request.name}» хочет подключиться к этому компьютеру.`);
					return __nsh_hv6;
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
		const __nsh_hv7 = element("view");
		prop(__nsh_hv7, "class", "h-full w-full");
		bindProp(__nsh_hv7, "style", () => ({ backgroundColor: colors().background }));
		const __nsh_hv8 = element("view");
		prop(__nsh_hv8, "class", "min-h-0 flex-1 flex-row");
		const __nsh_hv9 = element("view");
		prop(__nsh_hv9, "class", "h-full flex-col relative");
		prop(__nsh_hv9, "desktop", { windowDrag: true });
		const __nsh_hv10 = element("view");
		prop(__nsh_hv10, "class", "absolute inset-0");
		prop(__nsh_hv10, "desktop", { windowDrag: true });
		append(__nsh_hv9, __nsh_hv10);
		insert(__nsh_hv9, component(SettingsSidebar, {
			get colors() {
				return colors();
			},
			groups: navigation,
			get current() {
				return route();
			},
			get search() {
				return search();
			},
			onSearch: (value) => search.set(value),
			onNavigate: (next) => route.set(next)
		}));
		append(__nsh_hv8, __nsh_hv9);
		insert(__nsh_hv8, () => renderPage(route()));
		append(__nsh_hv7, __nsh_hv8);
		insert(__nsh_hv7, [component(Bridge, {
			ref: (node) => {
				session = { command: (name, args) => node.command(name, args) };
			},
			get route() {
				return route();
			},
			onState: (next) => state.set(next),
			onNavigate: (event) => route.set(event.route)
		}), component(TitleBar, { get colors() {
			return colors();
		} })], __nsh_hv8);
		insert(__nsh_hv7, [
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
		return __nsh_hv7;
	})();
}
mount(App);

//#endregion