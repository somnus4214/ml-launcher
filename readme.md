---
title: "Ml Launcher开发"
description: 
date: 2026-03-26T22:58:44+08:00
image: 
math: 
license: 
comments: true
draft: false
tags: ["Rust","Tool","Readme"]
build:
    list: always    # Change to "never" to hide the page from the list
---

>最近做一些模型训练的项目，每次在训练的时候，长长的后续参数都让人很头大（每次都要问a：怎么让这个项目跑起来），于是我想是否可以弄一个项目，方便的设置参数，便有了这个项目。

## 1.简单介绍
首先，为了保证模型训练的时候不能产生太多的内存和性能消耗，所以这个项目使用了rust来编写，实测这个项目的后台占用大概在20MB左右，几乎不会对模型训练产生影响。
其次，这个项目为了方便在终端运行，所以没有选用带GUI的前端，而是直接使用rust的终端显示库，ratatui，实际体验下来，这个库还是非常好用的，无论是api调用的**方便性**，还是**便捷内存占用**都是十分出色，非常适合我这个项目。

## 2.install
这个工具我已经上传crate.io，所以可以直接通过cargo install（没有安装rust环境的可以去装了:LOL ）
```zsh
cargo install ml-launcher
```

## 3.使用
下载到本地后会自动安装到bin目录，可以直接在终端中调用。
```zsh
ml-launcher
```
之后就可以打开这样的界面
![](https://somnusblog.oss-cn-shanghai.aliyuncs.com/images/20260327230007572.png)
按下p可以编辑想要识别python 参数的位置，按下c可以设置config.json的位置（这个一般可以通过进入程序后save config获取到）
随意选择一个进入方式，便进入了这样的界面
首先，第一个table，Parameters：就是主界面，用来对每个参数（可以选择部分参数）设置对应的值，同时右边也可以看见该参数需要的类型和描述。下方的一排是在当前table可以使用的快捷键。
![](https://somnusblog.oss-cn-shanghai.aliyuncs.com/images/20260327230351888.png)
其次，第二个table，filter：可以通过选择某个参数按t，来改变是否需要手动设置值（因为在大部分情况下，很多参数是不需要手动输入的，直接使用默认值就行）左边的Filter可以进行滚动显示。
![](https://somnusblog.oss-cn-shanghai.aliyuncs.com/images/20260327231856599.png)
最后，第三个table，actions：主要有下面这些可以执行的动作（当前版本直接run code好像是有问题的，而且不能选择conda环境或者uv环境中的python，所以建议复制指令使用）。
![](https://somnusblog.oss-cn-shanghai.aliyuncs.com/images/20260327231020211.png)
## 4.代码结构讲解
当前版本是"0.1.0"，所以后续如果有改动，该逻辑可能不准

### 4.1.modeselector
首先，刚进入程序会进入到模式选择，ModelSelector这个结构体是由以下元素组成的
```rust
pub struct ModeSelector {
    // 主菜单状态
    selected: usize, //作为选择的标志位。
    // 路径输入
    script_path: String, //输入的py文件地址，默认为当前目录下
    config_path: String,
    editing_script: bool, //设置一个bool值，方便表明该参数处于修改状态，会有不一样的颜色显示
    editing_config: bool,
    // 错误信息
    error_msg: String,
    // 退出标志
    should_quit: bool,
    // 继承配置对话框
    show_inherit_dialog: bool,
    inherit_selected: usize,
    inherit_config_path: String,
    editing_inherit_path: bool,
    // 合并结果展示
    show_merge_result: bool,
    pending_merge_result: Option<MergeResult>,
    pending_new_config: Option<TrainConfig>,
}
```
此处特地增加了一个should_quit的标志位，因为在没有这个标志位的时候，可能会因为两种不同的退出方式而产生冲突的bug。
ratatui设计的程序主要还有一个逻辑就是需要handle不同界面上的按键，分别有handle_main_menu_key，handle_inherit_dialog_key，handle_merge_result_key，handle_editing_script_key，handle_editing_config_key，handle_editing_inherit_path_key，分别对应不同的界面。
这个是第一部分的流程图。
![](https://somnusblog.oss-cn-shanghai.aliyuncs.com/images/ml-launcher-modelselector.drawio.png)
### 4.2主程序
如上面所介绍，主要分成三个table，其中，比较值得一说的是，主要有四个输入模式，
```rust
#[derive(Default, Clone, PartialEq)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
    SaveDialog,
    LoadDialog,
}
```
对于每个输入模式也有各自handle input的逻辑。每个handle input逻辑都需要处理不同按键和不同选择下的enter键。
每个参数都会有一个usedefault的布尔值，如果为ture，则不会再parameter中显示出来，反之则会显示，并且可以编辑对应的值。
```rust
        let active_params: Vec<(usize, &ParamField)> = self
            .config
            .params
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.use_default)
            .collect();
```
原本因为一个程序文件的总参数个数太多了，导致正常的终端的filter table根本没办法全部显示完，所以在这里改用了ListState来处理滚动显示逻辑。

```rust
let mut state = ListState::default();
        state.select(Some(self.selected_param));
```

## 5.小结

哎哎哎哎，其实还是有挺多话之的说的，但是不太想写了，我最近再做另一个todolist的app，希望那个能纯手搓出来，而不要用vide coding，hhhhhh，加油吧～～～。
